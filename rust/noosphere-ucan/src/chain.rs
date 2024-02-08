use crate::{
    capability::{
        proof::{ProofDelegationSemantics, ProofSelection},
        Ability, CapabilitySemantics, CapabilityView, Resource, ResourceUri, Scope,
    },
    crypto::did::DidParser,
    store::UcanJwtStore,
    ucan::Ucan,
};
use anyhow::{anyhow, Result};
use async_recursion::async_recursion;
use cid::Cid;
use std::{collections::BTreeSet, fmt::Debug};

const PROOF_DELEGATION_SEMANTICS: ProofDelegationSemantics = ProofDelegationSemantics {};

#[derive(Eq, PartialEq)]
pub struct CapabilityInfo<S: Scope, A: Ability> {
    pub originators: BTreeSet<String>,
    pub not_before: Option<u64>,
    pub expires_at: Option<u64>,
    pub capability: CapabilityView<S, A>,
}

impl<S, A> Debug for CapabilityInfo<S, A>
where
    S: Scope,
    A: Ability,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapabilityInfo")
            .field("originators", &self.originators)
            .field("not_before", &self.not_before)
            .field("expires_at", &self.expires_at)
            .field("capability", &self.capability)
            .finish()
    }
}

/// A deserialized chain of ancestral proofs that are linked to a UCAN
#[derive(Debug)]
pub struct ProofChain {
    ucan: Ucan,
    proofs: Vec<ProofChain>,
    redelegations: BTreeSet<usize>,
}

impl ProofChain {
    /// Instantiate a [ProofChain] from a [Ucan], given a [UcanJwtStore] and [DidParser]
    #[cfg_attr(target_arch = "wasm32", async_recursion(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_recursion)]
    pub async fn from_ucan<S>(
        ucan: Ucan,
        now_time: Option<u64>,
        did_parser: &mut DidParser,
        store: &S,
    ) -> Result<ProofChain>
    where
        S: UcanJwtStore,
    {
        ucan.validate(now_time, did_parser).await?;

        let mut proofs: Vec<ProofChain> = Vec::new();

        if let Some(ucan_proofs) = ucan.proofs() {
            for cid_string in ucan_proofs.iter() {
                let cid = Cid::try_from(cid_string.as_str())?;
                let ucan_token = store.require_token(&cid).await?;
                let proof_chain =
                    Self::try_from_token_string(&ucan_token, now_time, did_parser, store).await?;
                proof_chain.validate_link_to(&ucan)?;
                proofs.push(proof_chain);
            }
        }

        let mut redelegations = BTreeSet::<usize>::new();

        for capability in ucan
            .capabilities()
            .iter()
            .filter_map(|cap| PROOF_DELEGATION_SEMANTICS.parse_capability(&cap))
        {
            match capability.resource() {
                Resource::Resource {
                    kind: ResourceUri::Scoped(ProofSelection::All),
                } => {
                    for index in 0..proofs.len() {
                        redelegations.insert(index);
                    }
                }
                Resource::Resource {
                    kind: ResourceUri::Scoped(ProofSelection::Index(index)),
                } => {
                    if *index < proofs.len() {
                        redelegations.insert(*index);
                    } else {
                        return Err(anyhow!(
                            "Unable to redelegate proof; no proof at zero-based index {}",
                            index
                        ));
                    }
                }
                _ => continue,
            }
        }

        Ok(ProofChain {
            ucan,
            proofs,
            redelegations,
        })
    }

    /// Instantiate a [ProofChain] from a [Cid], given a [UcanJwtStore] and [DidParser]
    /// The [Cid] must resolve to a JWT token string
    pub async fn from_cid<S>(
        cid: &Cid,
        now_time: Option<u64>,
        did_parser: &mut DidParser,
        store: &S,
    ) -> Result<ProofChain>
    where
        S: UcanJwtStore,
    {
        Self::try_from_token_string(
            &store.require_token(cid).await?,
            now_time,
            did_parser,
            store,
        )
        .await
    }

    /// Instantiate a [ProofChain] from a JWT token string, given a [UcanJwtStore] and [DidParser]
    pub async fn try_from_token_string<'a, S>(
        ucan_token_string: &str,
        now_time: Option<u64>,
        did_parser: &mut DidParser,
        store: &S,
    ) -> Result<ProofChain>
    where
        S: UcanJwtStore,
    {
        let ucan = Ucan::try_from(ucan_token_string)?;
        Self::from_ucan(ucan, now_time, did_parser, store).await
    }

    fn validate_link_to(&self, ucan: &Ucan) -> Result<()> {
        let audience = self.ucan.audience();
        let issuer = ucan.issuer();

        match audience == issuer {
            true => match self.ucan.lifetime_encompasses(ucan) {
                true => Ok(()),
                false => Err(anyhow!("Invalid UCAN link: lifetime exceeds attenuation")),
            },
            false => Err(anyhow!(
                "Invalid UCAN link: audience {} does not match issuer {}",
                audience,
                issuer
            )),
        }
    }

    pub fn ucan(&self) -> &Ucan {
        &self.ucan
    }

    pub fn proofs(&self) -> &Vec<ProofChain> {
        &self.proofs
    }

    pub fn reduce_capabilities<Semantics, S, A>(
        &self,
        semantics: &Semantics,
    ) -> Vec<CapabilityInfo<S, A>>
    where
        Semantics: CapabilitySemantics<S, A>,
        S: Scope,
        A: Ability,
    {
        // Get the set of inherited attenuations (excluding redelegations)
        // before further attenuating by own lifetime and capabilities:
        let ancestral_capability_infos: Vec<CapabilityInfo<S, A>> = self
            .proofs
            .iter()
            .enumerate()
            .flat_map(|(index, ancestor_chain)| {
                if self.redelegations.contains(&index) {
                    Vec::new()
                } else {
                    ancestor_chain.reduce_capabilities(semantics)
                }
            })
            .collect();

        // Get the set of capabilities that are blanket redelegated from
        // ancestor proofs (via the prf: resource):
        let mut redelegated_capability_infos: Vec<CapabilityInfo<S, A>> = self
            .redelegations
            .iter()
            .flat_map(|index| {
                self.proofs
                    .get(*index)
                    .unwrap()
                    .reduce_capabilities(semantics)
                    .into_iter()
                    .map(|mut info| {
                        // Redelegated capabilities should be attenuated by
                        // this UCAN's lifetime
                        info.not_before = *self.ucan.not_before();
                        info.expires_at = *self.ucan.expires_at();
                        info
                    })
            })
            .collect();

        let self_capabilities_iter = self
            .ucan
            .capabilities()
            .iter()
            .map_while(|data| semantics.parse_capability(&data));

        // Get the claimed attenuations of this ucan, cross-checking ancestral
        // attenuations to discover the originating authority
        let mut self_capability_infos: Vec<CapabilityInfo<S, A>> = match self.proofs.len() {
            0 => self_capabilities_iter
                .map(|capability| CapabilityInfo {
                    originators: BTreeSet::from_iter(vec![self.ucan.issuer().to_string()]),
                    capability,
                    not_before: *self.ucan.not_before(),
                    expires_at: *self.ucan.expires_at(),
                })
                .collect(),
            _ => self_capabilities_iter
                .map(|capability| {
                    let mut originators = BTreeSet::<String>::new();

                    for ancestral_capability_info in ancestral_capability_infos.iter() {
                        match ancestral_capability_info.capability.enables(&capability) {
                            true => {
                                originators.extend(ancestral_capability_info.originators.clone())
                            }
                            // true => return Some(capability),
                            false => continue,
                        }
                    }

                    // If there are no related ancestral capability, then this
                    // link in the chain is considered the first originator
                    if originators.is_empty() {
                        originators.insert(self.ucan.issuer().to_string());
                    }

                    CapabilityInfo {
                        capability,
                        originators,
                        not_before: *self.ucan.not_before(),
                        expires_at: *self.ucan.expires_at(),
                    }
                })
                .collect(),
        };

        self_capability_infos.append(&mut redelegated_capability_infos);

        let mut merged_capability_infos = Vec::<CapabilityInfo<S, A>>::new();

        // Merge redundant capabilities (accounting for redelegation), ensuring
        // that discrete originators are aggregated as we go
        'merge: while let Some(capability_info) = self_capability_infos.pop() {
            for remaining_capability_info in &mut self_capability_infos {
                if remaining_capability_info
                    .capability
                    .enables(&capability_info.capability)
                {
                    remaining_capability_info
                        .originators
                        .extend(capability_info.originators);
                    continue 'merge;
                }
            }

            merged_capability_infos.push(capability_info);
        }

        merged_capability_infos
    }
}
