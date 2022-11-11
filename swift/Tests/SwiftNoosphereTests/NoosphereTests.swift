import XCTest
@testable import SwiftNoosphere

final class NoosphereTests: XCTestCase {
    func testInitializeNoosphereThenWriteAFileThenSaveThenReadItBack() throws {
        // This is a basic integration test to ensure that file writing and
        // reading from swift works as intended

        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil)

        ns_key_create(noosphere, "bob")

        let sphere_receipt = ns_sphere_create(noosphere, "bob")

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt)
        let sphere_mnemonic_ptr = ns_sphere_receipt_mnemonic(sphere_receipt)

        let sphere_identity = String.init(cString: sphere_identity_ptr!)
        let sphere_mnemonic = String.init(cString: sphere_mnemonic_ptr!)

        print("Sphere identity:", sphere_identity)
        print("Recovery code:", sphere_mnemonic)

        let sphere_fs = ns_sphere_fs_open(noosphere, sphere_identity_ptr)

        let file_bytes = "Hello, Subconscious".data(using: .utf8)!

        file_bytes.withUnsafeBytes({ (ptr: UnsafePointer<UInt8>) in
            let contents = slice_ref_uint8(ptr: ptr, len: file_bytes.count)
            ns_sphere_fs_write(noosphere, sphere_fs, "hello", "text/subtext", contents, nil)

            print("File write done")
        })

        ns_sphere_fs_save(noosphere, sphere_fs, nil)

        let file = ns_sphere_fs_read(noosphere, sphere_fs, "/hello")

        let content_type_values = ns_sphere_file_header_values_read(file, "Content-Type")
        let content_type = String.init(cString: content_type_values.ptr.pointee!)

        print("Content-Type:", content_type)

        let contents = ns_sphere_file_contents_read(noosphere, file)
        let data: Data = .init(bytes: contents.ptr, count: contents.len)
        let subtext = String.init(decoding: data, as: UTF8.self)

        print("Contents:", subtext)

        ns_string_array_free(content_type_values)
        ns_bytes_free(contents)
        ns_sphere_file_free(file)
        ns_sphere_fs_free(sphere_fs)
        ns_string_free(sphere_identity_ptr)
        ns_string_free(sphere_mnemonic_ptr)
        ns_sphere_receipt_free(sphere_receipt)
        ns_free(noosphere)

        print("fin!")
    }
}
