import XCTest

@testable import SwiftNoosphere

final class NoosphereTests: XCTestCase {
    func testInitializeNoosphereThenWriteAFileThenSaveThenReadItBack() throws {
        // This is a basic integration test to ensure that file writing and
        // reading from swift works as intended
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil, nil)

        ns_key_create(noosphere, "bob", nil)

        let sphere_receipt = ns_sphere_create(noosphere, "bob", nil)

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt, nil)
        let sphere_mnemonic_ptr = ns_sphere_receipt_mnemonic(sphere_receipt, nil)

        let sphere_identity = String.init(cString: sphere_identity_ptr!)
        let sphere_mnemonic = String.init(cString: sphere_mnemonic_ptr!)

        print("Sphere identity:", sphere_identity)
        print("Recovery code:", sphere_mnemonic)

        let sphere = ns_sphere_open(noosphere, sphere_identity_ptr, nil)

        let file_bytes = "Hello, Subconscious".data(using: .utf8)!

        file_bytes.withUnsafeBytes({ rawBufferPointer in
            let bufferPointer = rawBufferPointer.bindMemory(to: UInt8.self)
            let pointer = bufferPointer.baseAddress!
            let bodyRaw = slice_ref_uint8(
                ptr: pointer, len: file_bytes.count
            )
            ns_sphere_content_write(noosphere, sphere, "hello", "text/subtext", bodyRaw, nil, nil)
        })

        ns_sphere_save(noosphere, sphere, nil, nil)

        let file = ns_sphere_content_read_blocking(noosphere, sphere, "/hello", nil)

        let content_type_values = ns_sphere_file_header_values_read(file, "Content-Type")
        let content_type = String.init(cString: content_type_values.ptr.pointee!)

        print("Content-Type:", content_type)

        let contents = ns_sphere_file_contents_read_blocking(noosphere, file, nil)
        let data: Data = .init(bytes: contents.ptr, count: contents.len)
        let subtext = String.init(decoding: data, as: UTF8.self)

        print("Contents:", subtext)

        ns_string_array_free(content_type_values)
        ns_bytes_free(contents)
        ns_sphere_file_free(file)
        ns_sphere_free(sphere)
        ns_string_free(sphere_identity_ptr)
        ns_string_free(sphere_mnemonic_ptr)
        ns_sphere_receipt_free(sphere_receipt)
        ns_free(noosphere)

        print("fin!")
    }


    func testInitializeNoosphereThenWriteAFileThenSaveThenReadItBackWithACallback() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil, nil)

        ns_key_create(noosphere, "bob", nil)

        let sphere_receipt = ns_sphere_create(noosphere, "bob", nil)

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt, nil)
        let sphere_mnemonic_ptr = ns_sphere_receipt_mnemonic(sphere_receipt, nil)

        let sphere_identity = String.init(cString: sphere_identity_ptr!)
        let sphere_mnemonic = String.init(cString: sphere_mnemonic_ptr!)

        print("Sphere identity:", sphere_identity)
        print("Recovery code:", sphere_mnemonic)

        let sphere = ns_sphere_open(noosphere, sphere_identity_ptr, nil)

        let file_bytes = "Hello, Subconscious".data(using: .utf8)!

        let expectation = self.expectation(description: "File contents are read")

        file_bytes.withUnsafeBytes({ rawBufferPointer in
            let bufferPointer = rawBufferPointer.bindMemory(to: UInt8.self)
            let pointer = bufferPointer.baseAddress!
            let bodyRaw = slice_ref_uint8(
                ptr: pointer, len: file_bytes.count
            )
            ns_sphere_content_write(noosphere, sphere, "hello", "text/subtext", bodyRaw, nil, nil)
        })

        ns_sphere_save(noosphere, sphere, nil, nil)

        nsSphereContentRead(noosphere, sphere, "/hello") {
            (error, file) in

            if error != nil {
                let error_message_ptr = ns_error_string(error)
                let error_message = String.init(cString: error_message_ptr!)

                print(error_message)

                ns_string_free(error_message_ptr)
                ns_error_free(error)
                return
            }

            nsSphereFileContentsRead(noosphere, file) {
                (error, contents) in

                if error != nil {
                    let error_message_ptr = ns_error_string(error)
                    let error_message = String.init(cString: error_message_ptr!)

                    print(error_message)

                    ns_string_free(error_message_ptr)
                    ns_error_free(error)
                    return
                }

                let data: Data = .init(bytes: contents.ptr, count: contents.len)
                let subtext_from_callback = String.init(decoding: data, as: UTF8.self)

                assert("Hello, Subconscious" == subtext_from_callback)

                ns_bytes_free(contents)

                expectation.fulfill()
            }
        }

        self.waitForExpectations(timeout: 5)

        ns_sphere_free(sphere)
        ns_string_free(sphere_identity_ptr)
        ns_string_free(sphere_mnemonic_ptr)
        ns_sphere_receipt_free(sphere_receipt)
        ns_free(noosphere)
    }

    
    func testIterateOverAllHeadersForAFile() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil, nil)

        ns_key_create(noosphere, "bob", nil)

        let sphere_receipt = ns_sphere_create(noosphere, "bob", nil)

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt, nil)
        let sphere_mnemonic_ptr = ns_sphere_receipt_mnemonic(sphere_receipt, nil)

        let sphere = ns_sphere_open(noosphere, sphere_identity_ptr, nil)

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
            ns_sphere_content_write(noosphere, sphere, "hello", "text/subtext", bodyRaw, file_headers_in, nil)
        })

        ns_sphere_save(noosphere, sphere, nil, nil)

        let file = ns_sphere_content_read_blocking(noosphere, sphere, "/hello", nil)

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
        ns_sphere_free(sphere)
        ns_string_free(sphere_identity_ptr)
        ns_string_free(sphere_mnemonic_ptr)
        ns_sphere_receipt_free(sphere_receipt)
        ns_free(noosphere)

        print("fin!")
    }
    
    func testHandlingAnErrorCondition() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil, nil)

        let bad_sphere_identity = "doesnotexist"
        
        let maybe_error = UnsafeMutablePointer<OpaquePointer?>.allocate(capacity: 1)
        let sphere = ns_sphere_open(noosphere, bad_sphere_identity, maybe_error)
        
        assert(sphere == nil)
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
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil, nil)

        ns_key_create(noosphere, "bob", nil)

        let sphere_receipt = ns_sphere_create(noosphere, "bob", nil)

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt, nil)

        let sphere = ns_sphere_open(noosphere, sphere_identity_ptr, nil)

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
                        
                        ns_sphere_content_write(
                            noosphere,
                            sphere,
                            operation[1],
                            "text/subtext",
                            bodyRaw,
                            nil,
                            nil
                        )
                    })
                  case "remove":
                    ns_sphere_content_remove(noosphere, sphere, operation[1], nil)
                  default:
                    assert(false)
                }
            }
            
            ns_sphere_save(noosphere, sphere, nil, nil)
        
            let sphere_version_ptr = ns_sphere_version_get(noosphere, sphere_identity_ptr, nil)
            sphere_versions.append(String.init(cString: sphere_version_ptr!))
            ns_string_free(sphere_version_ptr)
        }
        
        let slugs = ns_sphere_content_list(noosphere, sphere, nil)
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
        ns_sphere_free(sphere)
        ns_free(noosphere)
    }
    
    func testGettingChangedSlugsAcrossRevisions() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil, nil)

        ns_key_create(noosphere, "bob", nil)

        let sphere_receipt = ns_sphere_create(noosphere, "bob", nil)

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt, nil)

        let sphere = ns_sphere_open(noosphere, sphere_identity_ptr, nil)

        let changes_to_make = [
            [
                ["add", "foo", "bar"],
                ["add", "baz", "vim"],
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
                        
                        ns_sphere_content_write(
                            noosphere,
                            sphere,
                            operation[1],
                            "text/subtext",
                            bodyRaw,
                            nil,
                            nil
                        )
                    })
                  case "remove":
                    ns_sphere_content_remove(noosphere, sphere, operation[1], nil)
                  default:
                    assert(false)
                }
            }
            
            ns_sphere_save(noosphere, sphere, nil, nil)
            let sphere_version_ptr = ns_sphere_version_get(noosphere, sphere_identity_ptr, nil)
            sphere_versions.append(String.init(cString: sphere_version_ptr!))
            ns_string_free(sphere_version_ptr)
        }
        
        let changes = ns_sphere_content_changes(noosphere, sphere, sphere_versions[0], nil)
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
        ns_sphere_free(sphere)
        ns_free(noosphere)
    }
    
    func testGettingVersionOfSphereFile() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil, nil)

        ns_key_create(noosphere, "bob", nil)

        let sphere_receipt = ns_sphere_create(noosphere, "bob", nil)

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt, nil)
        let sphere = ns_sphere_open(noosphere, sphere_identity_ptr, nil)
        
        ns_string_free(sphere_identity_ptr)
        ns_sphere_receipt_free(sphere_receipt)
        
        var file_bytes = "Hello, Subconscious".data(using: .utf8)!

        file_bytes.withUnsafeBytes({ rawBufferPointer in
            let bufferPointer = rawBufferPointer.bindMemory(to: UInt8.self)
            let pointer = bufferPointer.baseAddress!
            let bodyRaw = slice_ref_uint8(
                ptr: pointer, len: file_bytes.count
            )
            ns_sphere_content_write(noosphere, sphere, "hello", "text/subtext", bodyRaw, nil, nil)
        })

        ns_sphere_save(noosphere, sphere, nil, nil)

        var file = ns_sphere_content_read_blocking(noosphere, sphere, "/hello", nil)
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
            ns_sphere_content_write(noosphere, sphere, "hello", "text/subtext", bodyRaw, nil, nil)
        })

        ns_sphere_save(noosphere, sphere, nil, nil)

        file = ns_sphere_content_read_blocking(noosphere, sphere, "/hello", nil)
        version_ptr = ns_sphere_file_version_get(file, nil)
        let version_two = String.init(cString: version_ptr!)
        
        ns_sphere_file_free(file)
        
        assert(version_one != version_two)
        
        ns_sphere_free(sphere)
        ns_free(noosphere)
    }

    func testGettingIdentityFromSphere() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil, nil)

        ns_key_create(noosphere, "bob", nil)

        let sphere_receipt = ns_sphere_create(noosphere, "bob", nil)

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt, nil)
        let sphere = ns_sphere_open(noosphere, sphere_identity_ptr, nil)

        let reported_identity_str = ns_sphere_identity(noosphere, sphere, nil)

        assert(String.init(cString: reported_identity_str!) == String.init(cString: sphere_identity_ptr!))
        
        ns_string_free(sphere_identity_ptr)
        ns_sphere_receipt_free(sphere_receipt)
        ns_string_free(reported_identity_str)
        
        ns_sphere_free(sphere)
        ns_free(noosphere)
    }
    
    func testSettingAndGettingAPetname() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil, nil)

        ns_key_create(noosphere, "bob", nil)

        let sphere_receipt = ns_sphere_create(noosphere, "bob", nil)

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt, nil)
        let sphere = ns_sphere_open(noosphere, sphere_identity_ptr, nil)
        
        ns_string_free(sphere_identity_ptr)
        ns_sphere_receipt_free(sphere_receipt)
        
        ns_sphere_petname_set(noosphere, sphere, "alice", "did:key:alice", nil)
        ns_sphere_save(noosphere, sphere, nil, nil)
        
        let has_alice = ns_sphere_petname_is_set(noosphere, sphere, "alice", nil) == 1

        assert(has_alice)

        let identity_ptr = ns_sphere_petname_get(noosphere, sphere, "alice", nil)
        let identity = String.init(cString: identity_ptr!)
        
        assert(identity == "did:key:alice")

        // Unassign the petname alice
        ns_sphere_petname_set(noosphere, sphere, "alice", nil, nil)
        ns_sphere_save(noosphere, sphere, nil, nil)

        let has_alice_after_unassign = ns_sphere_petname_is_set(noosphere, sphere, "alice", nil) == 1
        assert(!has_alice_after_unassign)
        
        ns_string_free(identity_ptr)
        ns_sphere_free(sphere)
        ns_free(noosphere)
    }

    func testGetAllPetnamesInASphere() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil, nil)

        ns_key_create(noosphere, "bob", nil)

        let sphere_receipt = ns_sphere_create(noosphere, "bob", nil)

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt, nil)

        let sphere = ns_sphere_open(noosphere, sphere_identity_ptr, nil)

        let changes_to_make = [
            [
                ["add", "alice", "did:key:alice"],
                ["add", "cdata", "did:key:cdata"]
            ],
            [
                ["add", "bijaz", "did:key:bijaz"]
            ],
            [
                ["remove", "cdata"],
                ["add", "alice", "did:key:cdata"],
                ["add", "gordon", "did:key:gordon"]
            ]
        ]

        var sphere_versions: [String] = [];

        for i in 0..<changes_to_make.count {
            let revision = changes_to_make[i]
            
            for j in 0..<revision.count {
                let operation = revision[j]
                switch operation[0] {
                  case "add":
                    ns_sphere_petname_set(noosphere, sphere, operation[1], operation[2], nil)
                  case "remove":
                    ns_sphere_petname_set(noosphere, sphere, operation[1], nil, nil)
                  default:
                    assert(false)
                }
            }
            
            ns_sphere_save(noosphere, sphere, nil, nil)
        
            let sphere_version_ptr = ns_sphere_version_get(noosphere, sphere_identity_ptr, nil)
            sphere_versions.append(String.init(cString: sphere_version_ptr!))
            ns_string_free(sphere_version_ptr)
        }
        
        let petnames = ns_sphere_petname_list(noosphere, sphere, nil)
        let expected_petnames = [
            "alice",
            "bijaz",
            "gordon"
        ]
        
        let petname_count = petnames.len
        
        assert(petname_count == expected_petnames.count)
        
        var pointer = petnames.ptr!
        
        for i in 0..<petname_count {
            let petname = String.init(cString: pointer.pointee!)
            
            print("Petname:", petname)
            assert(expected_petnames[i] == petname)
            
            pointer += 1;
        }
        
        ns_string_array_free(petnames)
        ns_string_free(sphere_identity_ptr)
        ns_sphere_receipt_free(sphere_receipt)
        ns_sphere_free(sphere)
        ns_free(noosphere)
    }

    func testGettingChangedPetnamesAcrossRevisions() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil, nil)

        ns_key_create(noosphere, "bob", nil)

        let sphere_receipt = ns_sphere_create(noosphere, "bob", nil)

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt, nil)

        let sphere = ns_sphere_open(noosphere, sphere_identity_ptr, nil)

        let changes_to_make = [
            [
                ["add", "alice", "did:key:alice"],
                ["add", "cdata", "did:key:cdata"]
            ],
            [
                ["add", "bijaz", "did:key:bijaz"]
            ],
            [
                ["remove", "cdata"],
                ["add", "alice", "did:key:cdata"],
                ["add", "gordon", "did:key:gordon"]
            ]
        ]

        var sphere_versions: [String] = [];

        for i in 0..<changes_to_make.count {
            let revision = changes_to_make[i]
            
            for j in 0..<revision.count {
                let operation = revision[j]
                switch operation[0] {
                  case "add":
                    ns_sphere_petname_set(noosphere, sphere, operation[1], operation[2], nil)
                  case "remove":
                    ns_sphere_petname_set(noosphere, sphere, operation[1], nil, nil)
                  default:
                    assert(false)
                }
            }
            
            ns_sphere_save(noosphere, sphere, nil, nil)
        
            let sphere_version_ptr = ns_sphere_version_get(noosphere, sphere_identity_ptr, nil)
            sphere_versions.append(String.init(cString: sphere_version_ptr!))
            ns_string_free(sphere_version_ptr)
        }
        
        let changes = ns_sphere_petname_changes(noosphere, sphere, sphere_versions[0], nil)
        let expected_changes = [
            "alice",
            "bijaz",
            "cdata",
            "gordon"
        ]
        
        let change_count = changes.len
        
        assert(change_count == expected_changes.count)
        
        var pointer = changes.ptr!
        
        for i in 0..<change_count {
            let petname = String.init(cString: pointer.pointee!)
            
            print("Changed petname:", petname)
            assert(expected_changes[i] == petname)
            
            pointer += 1;
        }
        
        ns_string_array_free(changes)
        ns_string_free(sphere_identity_ptr)
        ns_sphere_receipt_free(sphere_receipt)
        ns_sphere_free(sphere)
        ns_free(noosphere)
    }

    // TODO(#315): Re-enable this test at some point
    /*
    func testDoesNotPanicWhenReadingProblematicSlashlinks() throws {
        let noosphere = ns_initialize("/tmp/foo", "/tmp/bar", nil, nil)

        ns_key_create(noosphere, "bob", nil)

        let sphere_receipt = ns_sphere_create(noosphere, "bob", nil)

        let sphere_identity_ptr = ns_sphere_receipt_identity(sphere_receipt, nil)

        let sphere = ns_sphere_open(noosphere, sphere_identity_ptr, nil)

        var maybe_error = UnsafeMutablePointer<OpaquePointer?>.allocate(capacity: 1)

        // Invalid slashlink
        var result = ns_sphere_content_read_blocking(noosphere, sphere, "cdata.dev/does-not-exist", maybe_error)
        
        assert(result == nil)
        assert(maybe_error.pointee != nil)

        let error_message_ptr = ns_error_string(maybe_error.pointee)
        let error_message = String.init(cString: error_message_ptr!)
        print(error_message)

        maybe_error.deallocate()
        maybe_error = UnsafeMutablePointer<OpaquePointer?>.allocate(capacity: 1)

        // Valid slashlink, unresolvable peer
        result = ns_sphere_content_read_blocking(noosphere, sphere, "@ben/does-not-exist", maybe_error)

        if maybe_error.pointee != nil {
            let error_message_ptr = ns_error_string(maybe_error.pointee)
            let error_message = String.init(cString: error_message_ptr!)
            print(error_message)
        }

        assert(result == nil)
        // NOTE: This assertion always fails on Github (and only on Github)
        assert(maybe_error.pointee == nil)

        maybe_error.deallocate()
        maybe_error = UnsafeMutablePointer<OpaquePointer?>.allocate(capacity: 1)

        // Valid slashlink, unresolvable slug
        result = ns_sphere_content_read(noosphere, sphere, "/does-not-exist", maybe_error)

        assert(result == nil)
        assert(maybe_error.pointee == nil)
    }
    */
}
