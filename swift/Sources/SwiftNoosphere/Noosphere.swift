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