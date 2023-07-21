//
//  NoosphereFuzz.swift
//  
//
//  Created by Christopher Joel on 7/21/23.
//
import XCTest

import SwiftNoosphere

final class NoosphereFuzzState {
    var currentNoosphere: OpaquePointer? = nil
    var currentKey: String? = nil
    var currentDid: String? = nil
    var currentSpheres: [OpaquePointer?] = []
    var currentSlugs: [String] = []
    
    init() {
        self.resetAndReinitializeNoosphere()
    }
    
    func appendId(_ toName: String) -> String {
        let id = Int64.random(in: 0..<Int64.max)
        return toName + String(id)
    }
    
    func assertNoError(_ error: OpaquePointer?) {
        if error != nil {
            let error_message_ptr = ns_error_message_get(error)
            let error_message = String.init(cString: error_message_ptr!)

            ns_string_free(error_message_ptr)
            ns_error_free(error)
            
            assert(false, error_message)
        }
    }
    
    func pickRandomFromStringArrayPointer(_ stringArray: slice_boxed_char_ptr_t) -> String? {
        if stringArray.len > 0 {
            let index = Int.random(in: 0..<stringArray.len)
            return String.init(cString: (stringArray.ptr! + index).pointee!)
        }
        return nil
    }
    
    func nsInitializeTemporary() -> OpaquePointer? {
        return ns_initialize(appendId("/tmp/foo"), appendId("/tmp/bar"), nil, nil)
    }
    
    func getRandomSphere() -> OpaquePointer? {
        if currentSpheres.isEmpty {
            createASphere()
        }
        
        return currentSpheres[Int.random(in: 0..<currentSpheres.count)]
    }
    
    func cleanupNoosphere() {
        for sphere in currentSpheres {
            ns_sphere_free(sphere)
        }
        
        currentSpheres = []
        currentSlugs = []
        
        if self.currentNoosphere != nil {
            print("Disposing current Noosphere...")
            ns_free(currentNoosphere)
        }
    }
    
    func resetAndReinitializeNoosphere() {
        cleanupNoosphere()
        
        print("Initializing Noosphere...")
        currentNoosphere = nsInitializeTemporary()
        currentKey = appendId("fuzz-key")
        currentDid = nil
        print("New key is", currentKey!)
        
        ns_key_create(currentNoosphere, currentKey!, nil)
    }
    
    func createASphere() {
        print("Creating a Sphere...")
        let sphere_receipt = ns_sphere_create(currentNoosphere, currentKey!, nil)
        let sphere_id = ns_sphere_receipt_identity(sphere_receipt, nil)
        let sphere = ns_sphere_open(currentNoosphere, sphere_id, nil)
        
        currentSpheres.append(sphere)
        
        if currentDid == nil {
            let didPtr = ns_sphere_author_identity(currentNoosphere, sphere, nil)
            currentDid = String.init(cString: didPtr!)
            ns_string_free(didPtr)
        }
        
        ns_string_free(sphere_id)
        ns_sphere_receipt_free(sphere_receipt)
    }
    
    func writeAFile(_ save: Bool = false) {
        let slug = appendId("slug")
        let sphere = getRandomSphere()
        print("Writing to slug", slug)
        let content = appendId("some content")
        let file_bytes = content.data(using: .utf8)!
        file_bytes.withUnsafeBytes({ rawBufferPointer in
            let bufferPointer = rawBufferPointer.bindMemory(to: UInt8.self)
            let pointer = bufferPointer.baseAddress!
            let bodyRaw = slice_ref_uint8(
                ptr: pointer, len: file_bytes.count
            )
            ns_sphere_content_write(currentNoosphere, sphere, slug, "text/plain", bodyRaw, nil, nil)
        })
        currentSlugs.append(slug)
        
        if save {
            print("Saving sphere...")
            ns_sphere_save(currentNoosphere, sphere, nil, nil)
        }
    }
    
    func removeAFile(_ save: Bool = false) {
        let sphere = getRandomSphere()
        let slugs = ns_sphere_content_list(currentNoosphere, sphere, nil)
        
        let slug = self.pickRandomFromStringArrayPointer(slugs)
        if slug != nil {
            print("Removing slug", slug!)
            
            ns_sphere_content_remove(currentNoosphere, sphere, slug, nil)
            
            if save {
                print("Saving sphere...")
                ns_sphere_save(currentNoosphere, sphere, nil, nil)
            }
        }
    }
    
    func addAnAuthorization(_ expectation: XCTestExpectation, _ save: Bool = false) {
        let sphere = getRandomSphere()
        let fakeDid = appendId("did:key:fake")
        let fakeName = appendId("fake")
        
        print("Authorizing", fakeDid)
        
        nsSphereAuthorityAuthorize(currentNoosphere, sphere, fakeName, fakeDid) {
            (error, authorization_ptr) in
            
            self.assertNoError(error)
            
            ns_string_free(authorization_ptr)

            if save {
                print("Saving sphere...")
                ns_sphere_save(self.currentNoosphere, sphere, nil, nil)
            }
            
            expectation.fulfill()
        }
    }
    
    func removeAnAuthorization(_ expectation: XCTestExpectation, _ save: Bool = false) {
        let sphere = getRandomSphere()
        
        nsSphereAuthorityAuthorizationsList(currentNoosphere, sphere) {
            (error, authorizations) in
            
            self.assertNoError(error)
            
            let authorization = self.pickRandomFromStringArrayPointer(authorizations!)
            
            if authorization != nil {
                
                nsSphereAuthorityAuthorizationIdentity(self.currentNoosphere, sphere, authorization) {
                    (error, identity_ptr) in
                    
                    self.assertNoError(error)
                    
                    let identity = String.init(cString: identity_ptr!)
                    ns_string_free(identity_ptr)
                    
                    if identity == self.currentDid {
                        expectation.fulfill()
                    } else {
                        print("Revoking authorization", authorization!)
                        
                        nsSphereAuthorityAuthorizationRevoke(self.currentNoosphere, sphere, authorization) {
                            (error) in
                            
                            self.assertNoError(error)
                            
                            if save {
                                print("Saving sphere...")
                                ns_sphere_save(self.currentNoosphere, sphere, nil, nil)
                            }
                            
                            expectation.fulfill()
                        }
                    }
                }
            }
        }
    }
}

final class NoosphereFuzz: XCTestCase {
    func testRandomFFIInvocations() throws {
        ns_tracing_initialize(NS_NOOSPHERE_LOG_DEAFENING.rawValue)
        
        let fuzz = NoosphereFuzzState.init()
        let invocations = [
            {
                fuzz.resetAndReinitializeNoosphere()
            },
            {
                fuzz.createASphere()
            },
            {
                fuzz.writeAFile(Bool.random())
            },
            {
                fuzz.removeAFile(Bool.random())
            },
            {
                fuzz.addAnAuthorization(self.expectation(description: "Authorization added"), Bool.random())
                self.waitForExpectations(timeout: 5)
            },
            {
                fuzz.removeAnAuthorization(self.expectation(description: "Authorization removed"), Bool.random())
                self.waitForExpectations(timeout: 5)
            }
        ]

        for _ in 0...1000 {
            let index = Int.random(in: 0..<invocations.count)
            let invocation = invocations[index]
            invocation()
        }
        
        fuzz.cleanupNoosphere()
    }
}
