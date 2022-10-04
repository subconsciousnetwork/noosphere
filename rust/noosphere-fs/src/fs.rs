use anyhow::{anyhow, Result};
use libipld_cbor::DagCborCodec;
use noosphere::{
    data::{BodyChunkIpld, ContentType, Header, MemoIpld},
    view::{Sphere, SphereMutation},
};
use noosphere_storage::{
    db::SphereDb,
    interface::{BlockStore, Store},
};
use std::str::FromStr;
use tokio_util::io::StreamReader;
use ucan::{crypto::KeyMaterial, ucan::Ucan};

use cid::Cid;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{BodyChunkDecoder, SphereFile};

/// SphereFs: An FS-like abstraction over Noosphere content.
///
/// A sphere implements a flat namespace that maps strings to CIDs, which in
/// turn refer to the sphere owner's content. However, it is not particularly
/// convenient for users to think of their content as organized around these
/// primitives. The SphereFs interface offers a familiar, relatively high-level
/// interface for operation on sphere content.
pub struct SphereFs<S>
where
    S: Store,
{
    sphere_identity: String,
    sphere_revision: Cid,
    db: SphereDb<S>,
}

impl<S> SphereFs<S>
where
    S: Store,
{
    pub fn identity(&self) -> &str {
        &self.sphere_identity
    }

    pub fn revision(&self) -> &Cid {
        &self.sphere_revision
    }

    pub fn to_sphere(&self) -> Sphere<S> {
        Sphere::at(self.revision(), &self.db.to_block_store())
    }

    async fn require_sphere_version(sphere_identity: &str, db: &SphereDb<S>) -> Result<Cid> {
        db.get_version(sphere_identity)
            .await?
            .ok_or_else(|| anyhow!("No reference to sphere {} found", sphere_identity))
    }

    async fn get_file(&self, memo_revision: &Cid) -> Result<SphereFile<impl AsyncRead + Unpin>> {
        let memo = self
            .db
            .load::<DagCborCodec, MemoIpld>(memo_revision)
            .await?;
        let content_type = match memo.get_first_header(&Header::ContentType.to_string()) {
            Some(content_type) => Some(ContentType::from_str(content_type.as_str())?),
            None => None,
        };

        let stream = match content_type {
            Some(ContentType::Subtext) | Some(ContentType::Bytes) => {
                BodyChunkDecoder(&memo.body, &self.db).stream()
            }
            Some(content_type) => {
                return Err(anyhow!("Unsupported content type: {}", content_type))
            }
            None => return Err(anyhow!("No content type specified")),
        };

        Ok(SphereFile {
            sphere_revision: self.sphere_revision,
            memo_revision: *memo_revision,
            memo,
            contents: StreamReader::new(stream),
        })
    }

    /// Create an FS view into the latest revision found in the provided sphere
    /// reference storage
    pub async fn latest(sphere_identity: &str, db: &SphereDb<S>) -> Result<SphereFs<S>> {
        let sphere_revision = Self::require_sphere_version(sphere_identity, db).await?;

        Ok(SphereFs {
            sphere_identity: sphere_identity.into(),
            sphere_revision,
            db: db.clone(),
        })
    }

    /// Create an FS view into the sphere data at a specific revision; note that
    /// writes to this view will "fork" history and update the sphere reference
    /// to point to the fork.
    pub fn at(
        sphere_identity: &str,
        sphere_revision: &Cid,
        db: &SphereDb<S>,
    ) -> Result<Option<SphereFs<S>>> {
        Ok(Some(SphereFs {
            sphere_identity: sphere_identity.into(),
            sphere_revision: *sphere_revision,
            db: db.clone(),
        }))
    }

    /// Rewind the view to point to the version of the sphere just prior to this
    /// one in the edit chronology. If there was a previous version to rewind to
    /// then the returned `Option` has the CID of the revision, otherwise if the
    /// current version is the oldest one it is `None`.
    pub async fn rewind(&mut self) -> Result<Option<Cid>> {
        let sphere = Sphere::at(&self.sphere_revision, &self.db);

        match sphere.try_get_parent().await? {
            Some(parent) => {
                self.sphere_revision = *parent.cid();
                Ok(Some(self.sphere_revision))
            }
            None => Ok(None),
        }
    }

    /// Returns true if the content identitifed by slug exists in the sphere at
    /// the current revision.
    pub async fn exists(&self, slug: &str) -> Result<bool> {
        Ok(self.read(slug).await?.is_some())
    }

    /// Read a file that is associated with a given slug at the revision of the
    /// sphere that this view is pointing to.
    /// Note that "contents" are `AsyncRead`, and content bytes won't be read
    /// until contents is polled.
    pub async fn read(&self, slug: &str) -> Result<Option<SphereFile<impl AsyncRead>>> {
        let sphere = Sphere::at(&self.sphere_revision, &self.db);
        let links = sphere.try_get_links().await?;
        let hamt = links.try_get_hamt().await?;

        Ok(match hamt.get(&slug.to_string()).await? {
            Some(content_cid) => Some(self.get_file(content_cid).await?),
            None => None,
        })
    }

    /// Write to a slug in the sphere. In addition to commiting new content to
    /// the sphere and block storage, this method:
    ///
    ///  - Creates a new revision based on the one that the view points to
    ///  - Signs the new revision with provided key material
    ///  - Updates the view to point to the new revision
    ///
    /// The returned CID is a link to the memo for the newly added content.
    pub async fn write<R: AsyncRead + std::marker::Unpin, K: KeyMaterial>(
        &mut self,
        slug: &str,
        content_type: &str,
        mut value: R,
        credential: &K,
        proof: Option<&Ucan>,
        additional_headers: Option<Vec<(String, String)>>,
    ) -> Result<Cid> {
        let current_file = self.read(slug).await?;
        let previous_memo_cid = current_file.map(|file| file.memo_revision);

        let sphere_cid = Self::require_sphere_version(&self.sphere_identity, &self.db).await?;
        let sphere = Sphere::at(&sphere_cid, &self.db);

        let mut bytes = Vec::new();
        value.read_to_end(&mut bytes).await?;

        // TODO(#38): We imply here that the only content types we care about
        // amount to byte streams, but in point of fact we can support anything
        // that may be referenced by CID including arbitrary IPLD structures
        let body_cid = BodyChunkIpld::store_bytes(&bytes, &mut self.db).await?;

        let mut new_memo = match previous_memo_cid {
            Some(cid) => {
                let mut memo = MemoIpld::branch_from(&cid, &self.db).await?;
                memo.body = body_cid;
                memo
            }
            None => MemoIpld {
                parent: None,
                headers: Vec::new(),
                body: body_cid,
            },
        };

        if let Some(headers) = additional_headers {
            for (name, value) in headers {
                new_memo.replace_header(&name, &value)
            }
        }

        new_memo.replace_header(&Header::ContentType.to_string(), content_type);

        // TODO(#43): Configure default/implicit headers here
        let memo_cid = self.db.save::<DagCborCodec, MemoIpld>(new_memo).await?;

        let author_did = credential.get_did().await?;

        let mut mutation = SphereMutation::new(&author_did);
        mutation.links_mut().set(&slug.into(), &memo_cid);

        let mut revision = sphere.try_apply_mutation(&mutation).await?;
        let next_sphere_cid = revision.try_sign(credential, proof).await?;

        self.db
            .set_version(&self.sphere_identity, &next_sphere_cid)
            .await?;

        self.sphere_revision = next_sphere_cid;

        Ok(memo_cid)
    }
}

#[cfg(test)]
pub mod tests {
    use noosphere::{
        authority::generate_ed25519_key,
        data::{ContentType, Header},
        view::Sphere,
    };
    use noosphere_storage::{db::SphereDb};
    use noosphere_storage::{memory::MemoryStorageProvider};
    use tokio::io::AsyncReadExt;
    use ucan::crypto::KeyMaterial;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use crate::SphereFs;

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_write_a_file_and_read_it_back() {
        let storage_provider = MemoryStorageProvider::default();
        let mut db = SphereDb::new(&storage_provider).await.unwrap();

        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, proof, _) = Sphere::try_generate(&owner_did, &mut db).await.unwrap();

        let sphere_identity = sphere.try_get_identity().await.unwrap();

        db.set_version(&sphere_identity, sphere.cid())
            .await
            .unwrap();

        let mut fs = SphereFs::latest(&sphere_identity, &db).await.unwrap();

        fs.write(
            "cats",
            &ContentType::Subtext.to_string(),
            b"Cats are great".as_ref(),
            &owner_key,
            Some(&proof),
            None,
        )
        .await
        .unwrap();

        let mut file = fs.read("cats").await.unwrap().unwrap();

        file.memo
            .expect_header(
                &Header::ContentType.to_string(),
                &ContentType::Subtext.to_string(),
            )
            .unwrap();

        let mut value = String::new();
        file.contents.read_to_string(&mut value).await.unwrap();

        assert_eq!("Cats are great", value.as_str());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_overwrite_a_file_with_new_contents_and_preserve_history() {
        let storage_provider = MemoryStorageProvider::default();
        let mut db = SphereDb::new(&storage_provider).await.unwrap();

        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, proof, _) = Sphere::try_generate(&owner_did, &mut db).await.unwrap();

        let sphere_identity = sphere.try_get_identity().await.unwrap();

        db.set_version(&sphere_identity, sphere.cid())
            .await
            .unwrap();

        let mut fs = SphereFs::latest(&sphere_identity, &db).await.unwrap();

        fs.write(
            "cats",
            &ContentType::Subtext.to_string(),
            b"Cats are great".as_ref(),
            &owner_key,
            Some(&proof),
            None,
        )
        .await
        .unwrap();

        fs.write(
            "cats",
            &ContentType::Subtext.to_string(),
            b"Cats are better than dogs".as_ref(),
            &owner_key,
            Some(&proof),
            None,
        )
        .await
        .unwrap();

        let mut file = fs.read("cats").await.unwrap().unwrap();

        file.memo
            .expect_header(
                &Header::ContentType.to_string(),
                &ContentType::Subtext.to_string(),
            )
            .unwrap();

        let mut value = String::new();
        file.contents.read_to_string(&mut value).await.unwrap();

        assert_eq!("Cats are better than dogs", value.as_str());

        assert!(fs.rewind().await.unwrap().is_some());

        file = fs.read("cats").await.unwrap().unwrap();

        file.memo
            .expect_header(
                &Header::ContentType.to_string(),
                &ContentType::Subtext.to_string(),
            )
            .unwrap();

        value.clear();
        file.contents.read_to_string(&mut value).await.unwrap();

        assert_eq!("Cats are great", value.as_str());
    }
}
