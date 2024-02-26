use noosphere_core::data::{Did, Link, LinkRecord, MemoIpld};

/// Various tasks that are performed by a job runner.
/// All jobs are scoped by an `identity` [Did], the
/// counterpart client sphere.
#[derive(Debug, Clone)]
pub enum GatewayJob {
    /// Compact history for the sphere.
    CompactHistory {
        /// Counterpart sphere associated with this job.
        identity: Did,
    },

    /// Syndicates blocks of the sphere to the broader IPFS network.
    IpfsSyndication {
        /// Counterpart sphere associated with this job.
        identity: Did,
        /// If provided, queues up a subsequent [GatewayJob::NameSystemPublish]
        /// job to run upon success with the provided link record.
        name_publish_on_success: Option<LinkRecord>,
    },

    /// Resolve all names in the sphere at the latest version.
    NameSystemResolveAll {
        /// Counterpart sphere associated with this job.
        identity: Did,
    },

    /// Resolve all added names of a given sphere since the given sphere
    /// revision.
    NameSystemResolveSince {
        /// Counterpart sphere associated with this job.
        identity: Did,
        /// Optional revision to resolve names since.
        since: Option<Link<MemoIpld>>,
    },

    /// Publish a link record (given as a [Jwt]) to the name system.
    NameSystemPublish {
        /// Counterpart sphere associated with this job.
        identity: Did,
        /// [LinkRecord] to publish.
        record: LinkRecord,
    },

    /// Republish the latest link record for a sphere.
    NameSystemRepublish {
        /// Counterpart sphere associated with this job.
        identity: Did,
    },
}
