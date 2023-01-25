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

        let sphere_fs = ns_sphere_fs_open(noosphere, sphere_identity_ptr, nil)

        let file_bytes = "Hello, Subconscious".data(using: .utf8)!

        file_bytes.withUnsafeBytes({ rawBufferPointer in
            let bufferPointer = rawBufferPointer.bindMemory(to: UInt8.self)
            let pointer = bufferPointer.baseAddress!
            let bodyRaw = slice_ref_uint8(
                ptr: pointer, len: file_bytes.count
            )
            ns_sphere_fs_write(noosphere, sphere_fs, "hello", "text/subtext", bodyRaw, nil)
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
    
    func testIterateOverAllHeadersForAFile() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil)

        ns_key_create(noosphere, "bob")

        let sphere_receipt = ns_sphere_create(noosphere, "bob")

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt)
        let sphere_mnemonic_ptr = ns_sphere_receipt_mnemonic(sphere_receipt)

        let sphere_fs = ns_sphere_fs_open(noosphere, sphere_identity_ptr, nil)

        let file_bytes = "Hello, Subconscious".data(using: .utf8)!
        let file_headers_in = ns_headers_create()
        
        ns_headers_add(file_headers_in, "foo", "bar")
        ns_headers_add(file_headers_in, "hello", "world")
        ns_headers_add(file_headers_in, "hello", "noosphere")

        file_bytes.withUnsafeBytes({ rawBufferPointer in
            let bufferPointer = rawBufferPointer.bindMemory(to: UInt8.self)
            let pointer = bufferPointer.baseAddress!
            let bodyRaw = slice_ref_uint8(
                ptr: pointer, len: file_bytes.count
            )
            ns_sphere_fs_write(noosphere, sphere_fs, "hello", "text/subtext", bodyRaw, file_headers_in)
        })

        ns_sphere_fs_save(noosphere, sphere_fs, nil)

        let file = ns_sphere_fs_read(noosphere, sphere_fs, "/hello")

        let file_header_names = ns_sphere_file_header_names_read(file)
        
        let name_count = file_header_names.len
        var pointer = file_header_names.ptr!;
        
        // NOTE: "hello" is only given once even though there are two headers with that name
        assert(name_count == 3)
        
        let expected_headers = [
            ["foo", "bar"],
            ["hello", "world"],
            ["Content-Type", "text/subtext"]
        ]
        
        for i in 0..<name_count {
            let name = String.init(cString: pointer.pointee!)
            let first_value_ptr = ns_sphere_file_header_value_first(file, name)
            let first_value = String.init(cString: first_value_ptr!)
            ns_string_free(first_value_ptr)
            
            print("Header name:", name)
            print("First value:", first_value)
            
            let expected = expected_headers[i]
            assert(name == expected[0])
            assert(first_value == expected[1])
            
            pointer += 1;
        }

        ns_string_array_free(file_header_names)
        ns_headers_free(file_headers_in)
        ns_sphere_file_free(file)
        ns_sphere_fs_free(sphere_fs)
        ns_string_free(sphere_identity_ptr)
        ns_string_free(sphere_mnemonic_ptr)
        ns_sphere_receipt_free(sphere_receipt)
        ns_free(noosphere)

        print("fin!")
    }
    
    func testHandlingAnErrorCondition() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil)

        let bad_sphere_identity = "doesnotexist"
        
        let maybe_error = UnsafeMutablePointer<OpaquePointer?>.allocate(capacity: 1)
        let sphere_fs = ns_sphere_fs_open(noosphere, bad_sphere_identity, maybe_error)
        
        assert(sphere_fs == nil)
        assert(maybe_error.pointee != nil)
        
        let error_message_ptr = ns_error_string(maybe_error.pointee)
        let error_message = String.init(cString: error_message_ptr!)
        
        print(error_message)
        assert(!error_message.isEmpty)
        
        ns_string_free(error_message_ptr)
        ns_error_free(maybe_error.pointee)
        ns_free(noosphere)
    }
    
    func testGetAllSlugsInASphere() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil)

        ns_key_create(noosphere, "bob")

        let sphere_receipt = ns_sphere_create(noosphere, "bob")

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt)

        let sphere_fs = ns_sphere_fs_open(noosphere, sphere_identity_ptr, nil)

        let changes_to_make = [
            [
                ["add", "foo", "bar"],
                ["add", "hello", "world"]
            ],
            [
                ["add", "bijaz", "tleilax"]
            ],
            [
                ["remove", "foo"],
                ["add", "hello", "noosphere"],
                ["add", "fizz", "buzz"]
            ]
        ]

        var sphere_versions: [String] = [];

        for i in 0..<changes_to_make.count {
            let revision = changes_to_make[i]
            
            for j in 0..<revision.count {
                let operation = revision[j]
                switch operation[0] {
                  case "add":
                    let file_bytes = operation[2].data(using: .utf8)!
                    file_bytes.withUnsafeBytes({ rawBufferPointer in
                        let bufferPointer = rawBufferPointer.bindMemory(to: UInt8.self)
                        let pointer = bufferPointer.baseAddress!
                        let bodyRaw = slice_ref_uint8(
                            ptr: pointer, len: file_bytes.count
                        )
                        
                        ns_sphere_fs_write(
                            noosphere,
                            sphere_fs,
                            operation[1],
                            "text/subtext",
                            bodyRaw,
                            nil
                        )
                    })
                  case "remove":
                    ns_sphere_fs_remove(noosphere, sphere_fs, operation[1], nil)
                  default:
                    assert(false)
                }
            }
            
            ns_sphere_fs_save(noosphere, sphere_fs, nil)
        
            let sphere_version_ptr = ns_sphere_version_get(noosphere, sphere_identity_ptr, nil)
            sphere_versions.append(String.init(cString: sphere_version_ptr!))
            ns_string_free(sphere_version_ptr)
        }
        
        let slugs = ns_sphere_fs_list(noosphere, sphere_fs, nil)
        let expected_slugs = [
            "bijaz",
            "fizz",
            "hello"
        ]
        
        let slug_count = slugs.len
        
        assert(slug_count == expected_slugs.count)
        
        var pointer = slugs.ptr!
        
        for i in 0..<slug_count {
            let slug = String.init(cString: pointer.pointee!)
            
            print("Slug:", slug)
            assert(expected_slugs[i] == slug)
            
            pointer += 1;
        }
        
        ns_string_array_free(slugs)
        ns_string_free(sphere_identity_ptr)
        ns_sphere_receipt_free(sphere_receipt)
        ns_sphere_fs_free(sphere_fs)
        ns_free(noosphere)
    }
    
    func testGettingChangedSlugsAcrossRevisions() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil)

        ns_key_create(noosphere, "bob")

        let sphere_receipt = ns_sphere_create(noosphere, "bob")

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt)

        let sphere_fs = ns_sphere_fs_open(noosphere, sphere_identity_ptr, nil)

        let changes_to_make = [
            [
                ["add", "foo", "bar"],
                ["add", "hello", "world"]
            ],
            [
                ["add", "bijaz", "tleilax"]
            ],
            [
                ["remove", "foo"],
                ["add", "hello", "noosphere"],
                ["add", "fizz", "buzz"]
            ]
        ]

        var sphere_versions: [String] = [];

        for i in 0..<changes_to_make.count {
            let revision = changes_to_make[i]
            
            for j in 0..<revision.count {
                let operation = revision[j]
                switch operation[0] {
                  case "add":
                    let file_bytes = operation[2].data(using: .utf8)!
                    file_bytes.withUnsafeBytes({ rawBufferPointer in
                        let bufferPointer = rawBufferPointer.bindMemory(to: UInt8.self)
                        let pointer = bufferPointer.baseAddress!
                        let bodyRaw = slice_ref_uint8(
                            ptr: pointer, len: file_bytes.count
                        )
                        
                        ns_sphere_fs_write(
                            noosphere,
                            sphere_fs,
                            operation[1],
                            "text/subtext",
                            bodyRaw,
                            nil
                        )
                    })
                  case "remove":
                    ns_sphere_fs_remove(noosphere, sphere_fs, operation[1], nil)
                  default:
                    assert(false)
                }
            }
            
            ns_sphere_fs_save(noosphere, sphere_fs, nil)
            let sphere_version_ptr = ns_sphere_version_get(noosphere, sphere_identity_ptr, nil)
            sphere_versions.append(String.init(cString: sphere_version_ptr!))
            ns_string_free(sphere_version_ptr)
        }
        
        let changes = ns_sphere_fs_changes(noosphere, sphere_fs, sphere_versions[0], nil)
        let expected_changes = [
            "bijaz",
            "fizz",
            "foo",
            "hello"
        ]
        
        let change_count = changes.len
        
        assert(change_count == expected_changes.count)
        
        var pointer = changes.ptr!
        
        for i in 0..<change_count {
            let slug = String.init(cString: pointer.pointee!)
            
            print("Changed slug:", slug)
            assert(expected_changes[i] == slug)
            
            pointer += 1;
        }
        
        ns_string_array_free(changes)
        ns_string_free(sphere_identity_ptr)
        ns_sphere_receipt_free(sphere_receipt)
        ns_sphere_fs_free(sphere_fs)
        ns_free(noosphere)
    }
    
    func testGettingVersionOfSphereFile() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil)

        ns_key_create(noosphere, "bob")

        let sphere_receipt = ns_sphere_create(noosphere, "bob")

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt)
        let sphere_fs = ns_sphere_fs_open(noosphere, sphere_identity_ptr, nil)
        
        ns_string_free(sphere_identity_ptr)
        ns_sphere_receipt_free(sphere_receipt)
        
        var file_bytes = "Hello, Subconscious".data(using: .utf8)!

        file_bytes.withUnsafeBytes({ rawBufferPointer in
            let bufferPointer = rawBufferPointer.bindMemory(to: UInt8.self)
            let pointer = bufferPointer.baseAddress!
            let bodyRaw = slice_ref_uint8(
                ptr: pointer, len: file_bytes.count
            )
            ns_sphere_fs_write(noosphere, sphere_fs, "hello", "text/subtext", bodyRaw, nil)
        })

        ns_sphere_fs_save(noosphere, sphere_fs, nil)

        var file = ns_sphere_fs_read(noosphere, sphere_fs, "/hello")
        var version_ptr = ns_sphere_file_version_get(file, nil)
        let version_one = String.init(cString: version_ptr!)
        
        ns_string_free(version_ptr)
        ns_sphere_file_free(file)
        
        file_bytes = "Hello, Noosphere".data(using: .utf8)!

        file_bytes.withUnsafeBytes({ rawBufferPointer in
            let bufferPointer = rawBufferPointer.bindMemory(to: UInt8.self)
            let pointer = bufferPointer.baseAddress!
            let bodyRaw = slice_ref_uint8(
                ptr: pointer, len: file_bytes.count
            )
            ns_sphere_fs_write(noosphere, sphere_fs, "hello", "text/subtext", bodyRaw, nil)
        })

        ns_sphere_fs_save(noosphere, sphere_fs, nil)

        file = ns_sphere_fs_read(noosphere, sphere_fs, "/hello")
        version_ptr = ns_sphere_file_version_get(file, nil)
        let version_two = String.init(cString: version_ptr!)
        
        ns_sphere_file_free(file)
        
        assert(version_one != version_two)
        
        ns_sphere_fs_free(sphere_fs)
        ns_free(noosphere)
    }
}
