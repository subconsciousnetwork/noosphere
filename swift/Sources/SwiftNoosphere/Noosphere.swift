@_exported import Noosphere

/// A container used in the these API helpers to represent a closure as an
/// object so that it can be passed back and forth through FFI as a pointer
class Box<Contents> {
    let contents: Contents

    init(contents: Contents) {
        self.contents = contents
    }
}


public typealias NsSphereContentReadHandler = (OpaquePointer?, OpaquePointer?) -> ()

/// See: ns_sphere_content_read
public func nsSphereContentRead(_ noosphere: OpaquePointer!, _ sphere: OpaquePointer!, _ slashlink: UnsafePointer<CChar>!, handler: @escaping NsSphereContentReadHandler) {
    let context = Unmanaged.passRetained(Box(contents: handler)).toOpaque()

    ns_sphere_content_read(noosphere, sphere, slashlink, context) {
        (context, error, file) in

        guard let context = context else {
            return
        }

        let handler = Unmanaged<Box<NsSphereContentReadHandler>>.fromOpaque(context).takeRetainedValue()

        handler.contents(error, file)
    }
}


public typealias NsSphereFileContentsReadHandler = (OpaquePointer?, slice_boxed_uint8_t) -> ()

/// See: ns_sphere_file_contents_read
public func nsSphereFileContentsRead(_ noosphere: OpaquePointer!, _ sphere_file: OpaquePointer!, handler: @escaping NsSphereFileContentsReadHandler) {
    let context = Unmanaged.passRetained(Box(contents: handler)).toOpaque()

    ns_sphere_file_contents_read(noosphere, sphere_file, context) {
        (context, error, bytes) in

        guard let context = context else {
            return
        }

        let handler = Unmanaged<Box<NsSphereFileContentsReadHandler>>.fromOpaque(context).takeRetainedValue()

        handler.contents(error, bytes)
    }
}


public typealias NsSpherePetnamesAssignedGetHandler = (OpaquePointer?, slice_boxed_char_ptr_t) -> ()

/// See: ns_sphere_petnames_assigned_get
public func nsSpherePetnamesAssignedGet(_ noosphere: OpaquePointer!, _ sphere_file: OpaquePointer!, _ peer_identity: UnsafePointer<CChar>!, handler: @escaping NsSpherePetnamesAssignedGetHandler) {
    let context = Unmanaged.passRetained(Box(contents: handler)).toOpaque()

    ns_sphere_petnames_assigned_get(noosphere, sphere_file, peer_identity, context) {
        (context, error, petnames) in

        guard let context = context else {
            return
        }

        let handler = Unmanaged<Box<NsSpherePetnamesAssignedGetHandler>>.fromOpaque(context).takeRetainedValue()

        handler.contents(error, petnames)
    }
}


public typealias NsSphereTraverseByPetnameHandler = (OpaquePointer?, OpaquePointer?) -> ()

/// See: ns_sphere_traverse_by_petname
public func nsSphereTraverseByPetname(_ noosphere: OpaquePointer!, _ sphere: OpaquePointer!, _ petname: UnsafePointer<CChar>!, handler: @escaping NsSphereTraverseByPetnameHandler) {
    let context = Unmanaged.passRetained(Box(contents: handler)).toOpaque()

    ns_sphere_traverse_by_petname(noosphere, sphere, petname, context) {
        (context, error, sphere) in

        guard let context = context else {
            return
        }

        let handler = Unmanaged<Box<NsSphereContentReadHandler>>.fromOpaque(context).takeRetainedValue()

        handler.contents(error, sphere)
    }
}


public typealias NsSphereSyncHandler = (OpaquePointer?, UnsafeMutablePointer<CChar>?) -> ()

/// See: ns_sphere_sync
public func nsSphereSync(_ noosphere: OpaquePointer!, _ sphere: OpaquePointer!, handler: @escaping NsSphereSyncHandler) {
    let context = Unmanaged.passRetained(Box(contents: handler)).toOpaque()

    ns_sphere_sync(noosphere, sphere, context) {
        (context, error, new_version) in

        guard let context = context else {
            return
        }

        let handler = Unmanaged<Box<NsSphereSyncHandler>>.fromOpaque(context).takeRetainedValue()

        handler.contents(error, new_version)
    }
}

public typealias NsSphereAuthorityEscalateHandler = (OpaquePointer?, OpaquePointer?) -> ()

/// See: ns_sphere_authority_escalate
public func nsSphereAuthorityEscalate(_ noosphere: OpaquePointer!, _ sphere: OpaquePointer!, _ mnemonic: UnsafePointer<CChar>!, handler: @escaping NsSphereAuthorityEscalateHandler) {
    let context = Unmanaged.passRetained(Box(contents: handler)).toOpaque()

    ns_sphere_authority_escalate(noosphere, sphere, mnemonic, context) {
        (context, error, escalated_sphere) in

        guard let context = context else {
            return
        }

        let handler = Unmanaged<Box<NsSphereAuthorityEscalateHandler>>.fromOpaque(context).takeRetainedValue()

        handler.contents(error, escalated_sphere)
    }
}

public typealias NsSphereAuthorityAuthorizeHandler = (OpaquePointer?, UnsafeMutablePointer<CChar>?) -> ()

/// See: ns_sphere_authority_authorize
public func nsSphereAuthorityAuthorize(_ noosphere: OpaquePointer!, _ sphere: OpaquePointer!, _ name: UnsafePointer<CChar>!, _ did: UnsafePointer<CChar>!, handler: @escaping NsSphereAuthorityAuthorizeHandler) {
    let context = Unmanaged.passRetained(Box(contents: handler)).toOpaque()

    ns_sphere_authority_authorize(noosphere, sphere, name, did, context) {
        (context, error, authorization) in

        guard let context = context else {
            return
        }

        let handler = Unmanaged<Box<NsSphereAuthorityAuthorizeHandler>>.fromOpaque(context).takeRetainedValue()

        handler.contents(error, authorization)
    }
}

public typealias NsSphereAuthorityAuthorizationRevokeHandler = (OpaquePointer?) -> ()

/// See: ns_sphere_authority_authorization_revoke
public func nsSphereAuthorityAuthorizationRevoke(_ noosphere: OpaquePointer!, _ sphere: OpaquePointer!, _ authorization: UnsafePointer<CChar>!, handler: @escaping NsSphereAuthorityAuthorizationRevokeHandler) {
    let context = Unmanaged.passRetained(Box(contents: handler)).toOpaque()

    ns_sphere_authority_authorization_revoke(noosphere, sphere, authorization, context) {
        (context, error) in

        guard let context = context else {
            return
        }

        let handler = Unmanaged<Box<NsSphereAuthorityAuthorizationRevokeHandler>>.fromOpaque(context).takeRetainedValue()

        handler.contents(error)
    }
}
