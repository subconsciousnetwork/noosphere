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
use once_cell::sync::OnceCell;
use std::{str::FromStr, sync::Once};
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
    author_identity: Option<String>,
    sphere_identity: String,
    sphere_revision: Cid,
    db: SphereDb<S>,
    mutation: OnceCell<SphereMutation>,
}

impl<S> SphereFs<S>
where
    S: Store,
{
    /// Get the DID identity of the sphere that this FS view is reading from and
    /// writing to
    pub fn identity(&self) -> &str {
        &self.sphere_identity
    }

    /// The CID revision of the sphere that this FS view is reading from and
    /// writing to
    pub fn revision(&self) -> &Cid {
        &self.sphere_revision
    }

    /// Get a data view into the sphere at the current revision
    pub fn to_sphere(&self) -> Sphere<S> {
        Sphere::at(self.revision(), &self.db.to_block_store())
    }

    fn require_author(&self) -> Result<&str> {
        Ok(self.author_identity.as_ref().ok_or_else(|| {
            anyhow!("You must initialize the sphere FS view with an author DID in order to write to the sphere")
        })?)
    }

    fn require_mutation(&mut self) -> Result<&mut SphereMutation> {
        let author_identity = self.require_author()?;
        self.mutation
            .get_or_init(|| SphereMutation::new(author_identity));

        self.mutation
            .get_mut()
            .ok_or_else(|| anyhow!("Failed to initialize sphere mutation"))
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
            // TODO(#86): Content-type aware decoding of body bytes
            Some(_) => BodyChunkDecoder(&memo.body, &self.db).stream(),
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
    pub async fn latest(
        sphere_identity: &str,
        author_identity: Option<&str>,
        db: &SphereDb<S>,
    ) -> Result<SphereFs<S>> {
        let sphere_revision = db.require_version(sphere_identity).await?;

        Ok(SphereFs {
            sphere_identity: sphere_identity.into(),
            sphere_revision,
            author_identity: author_identity.map(|did| did.into()),
            db: db.clone(),
            mutation: OnceCell::new(),
        })
    }

    /// Create an FS view into the sphere data at a specific revision; note that
    /// writes to this view will "fork" history and update the sphere reference
    /// to point to the fork.
    pub fn at(
        sphere_identity: &str,
        sphere_revision: &Cid,
        author_identity: Option<&str>,
        db: &SphereDb<S>,
    ) -> SphereFs<S> {
        SphereFs {
            sphere_identity: sphere_identity.into(),
            sphere_revision: *sphere_revision,
            author_identity: author_identity.map(|did| did.into()),
            db: db.clone(),
            mutation: OnceCell::new(),
        }
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

    /// Write to a slug in the sphere. In order to commit the change to the
    /// sphere, you must call save. You can buffer multiple writes before
    /// saving.
    ///
    /// The returned CID is a link to the memo for the newly added content.
    pub async fn write<R: AsyncRead + std::marker::Unpin>(
        &mut self,
        slug: &str,
        content_type: &str,
        mut value: R,
        additional_headers: Option<Vec<(String, String)>>,
    ) -> Result<Cid> {
        self.require_author()?;

        let mut bytes = Vec::new();
        value.read_to_end(&mut bytes).await?;

        // TODO(#38): We imply here that the only content types we care about
        // amount to byte streams, but in point of fact we can support anything
        // that may be referenced by CID including arbitrary IPLD structures
        let body_cid = BodyChunkIpld::store_bytes(&bytes, &mut self.db).await?;

        self.link(slug, content_type, &body_cid, additional_headers)
            .await
    }

    /// Similar to write, but instead of generating blocks from some provided
    /// bytes, the caller provides a CID of an existing DAG in storage. That
    /// CID is used as the body of a Memo that is written to the specified
    /// slug, and the CID of the memo is returned.
    pub async fn link(
        &mut self,
        slug: &str,
        content_type: &str,
        body_cid: &Cid,
        additional_headers: Option<Vec<(String, String)>>,
    ) -> Result<Cid> {
        self.require_author()?;

        let current_file = self.read(slug).await?;
        let previous_memo_cid = current_file.map(|file| file.memo_revision);

        let mut new_memo = match previous_memo_cid {
            Some(cid) => {
                let mut memo = MemoIpld::branch_from(&cid, &self.db).await?;
                memo.body = *body_cid;
                memo
            }
            None => MemoIpld {
                parent: None,
                headers: Vec::new(),
                body: *body_cid,
            },
        };

        if let Some(mut headers) = additional_headers {
            new_memo.headers.append(&mut headers)
        }

        new_memo.replace_header(&Header::ContentType.to_string(), content_type);

        // TODO(#43): Configure default/implicit headers here
        let memo_cid = self.db.save::<DagCborCodec, MemoIpld>(new_memo).await?;

        let mutation = self.require_mutation()?;
        mutation.links_mut().set(&slug.into(), &memo_cid);

        Ok(memo_cid)
    }

    /// Unlinks a slug from the content space. Note that this does not remove
    /// the blocks that were previously associated with the content found at the
    /// given slug, because they will still be available at an earlier revision
    /// of the sphere. In order to commit the change, you must save. Note that
    /// this call is a no-op if there is no matching slug linked in the sphere.
    ///
    /// The returned value is the CID previously associated with the slug, if
    /// any.
    pub async fn remove(&mut self, slug: &str) -> Result<Option<Cid>> {
        let current_file = self.read(slug).await?;
        Ok(match current_file {
            Some(file) => {
                let mutation = self.require_mutation()?;
                mutation.links_mut().remove(&String::from(slug));

                Some(file.memo_revision)
            }
            None => None,
        })
    }

    /// Commits a series of writes to the sphere. In addition to commiting new
    /// content to the sphere and block storage, this method:
    ///
    ///  - Creates a new revision based on the one that this FS view points to
    ///  - Signs the new revision with provided key material
    ///  - Updates this FS view to point to the new revision
    ///
    /// The new revision CID of the sphere is returned.
    pub async fn save<K: KeyMaterial>(
        &mut self,
        credential: &K,
        proof: Option<&Ucan>,
        additional_headers: Option<Vec<(String, String)>>,
    ) -> Result<Cid> {
        let sphere = Sphere::at(&self.sphere_revision, &self.db);

        // TODO(#85): Error result if mutation is empty and also additional headers are empty

        let mut revision = sphere.try_apply_mutation(self.require_mutation()?).await?;

        if let Some(mut headers) = additional_headers {
            revision.memo.headers.append(&mut headers);
        }

        let new_sphere_revision = revision.try_sign(credential, proof).await?;

        self.db
            .set_version(&self.sphere_identity, &new_sphere_revision)
            .await?;
        self.sphere_revision = new_sphere_revision.clone();
        self.mutation = OnceCell::new();

        Ok(new_sphere_revision)
    }
}

#[cfg(test)]
pub mod tests {
    use noosphere::{
        authority::generate_ed25519_key,
        data::{ContentType, Header},
        view::Sphere,
    };
    use noosphere_storage::db::SphereDb;
    use noosphere_storage::memory::MemoryStorageProvider;
    use tokio::io::AsyncReadExt;
    use ucan::crypto::KeyMaterial;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use crate::SphereFs;

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_unlink_slugs_from_the_content_space() {
        let storage_provider = MemoryStorageProvider::default();
        let mut db = SphereDb::new(&storage_provider).await.unwrap();

        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, proof, _) = Sphere::try_generate(&owner_did, &mut db).await.unwrap();

        let sphere_identity = sphere.try_get_identity().await.unwrap();

        db.set_version(&sphere_identity, sphere.cid())
            .await
            .unwrap();

        let mut fs = SphereFs::latest(&sphere_identity, Some(&owner_did), &db)
            .await
            .unwrap();

        fs.write(
            "cats",
            &ContentType::Subtext.to_string(),
            b"Cats are great".as_ref(),
            None,
        )
        .await
        .unwrap();

        fs.save(&owner_key, Some(&proof), None).await.unwrap();

        assert!(fs.read("cats").await.unwrap().is_some());

        fs.remove("cats").await.unwrap();
        fs.save(&owner_key, Some(&proof), None).await.unwrap();

        assert!(fs.read("cats").await.unwrap().is_none());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_does_not_allow_writes_when_no_author_is_configured() {
        let storage_provider = MemoryStorageProvider::default();
        let mut db = SphereDb::new(&storage_provider).await.unwrap();

        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, proof, _) = Sphere::try_generate(&owner_did, &mut db).await.unwrap();

        let sphere_identity = sphere.try_get_identity().await.unwrap();

        db.set_version(&sphere_identity, sphere.cid())
            .await
            .unwrap();

        let mut fs = SphereFs::latest(&sphere_identity, None, &db).await.unwrap();

        let write_result = fs
            .write(
                "cats",
                &ContentType::Subtext.to_string(),
                b"Cats are great".as_ref(),
                None,
            )
            .await;

        assert!(write_result.is_err());
    }

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

        let mut fs = SphereFs::latest(&sphere_identity, Some(&owner_did), &db)
            .await
            .unwrap();

        fs.write(
            "cats",
            &ContentType::Subtext.to_string(),
            b"Cats are great".as_ref(),
            None,
        )
        .await
        .unwrap();

        fs.save(&owner_key, Some(&proof), None).await.unwrap();

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

        let mut fs = SphereFs::latest(&sphere_identity, Some(&owner_did), &db)
            .await
            .unwrap();

        fs.write(
            "cats",
            &ContentType::Subtext.to_string(),
            b"Cats are great".as_ref(),
            None,
        )
        .await
        .unwrap();

        fs.save(&owner_key, Some(&proof), None).await.unwrap();

        fs.write(
            "cats",
            &ContentType::Subtext.to_string(),
            b"Cats are better than dogs".as_ref(),
            None,
        )
        .await
        .unwrap();

        fs.save(&owner_key, Some(&proof), None).await.unwrap();

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
