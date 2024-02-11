// Copyright 2020 Contributors to the Parsec project.
// SPDX-License-Identifier: Apache-2.0
use super::{FailingMockIpc, TestBasicClient, DEFAULT_APP_NAME};
use crate::auth::Authentication;
use crate::error::{ClientErrorKind, Error};
use mockstream::{FailingMockStream, MockStream};
use parsec_interface::operations;
use parsec_interface::operations::list_authenticators::AuthenticatorInfo;
use parsec_interface::operations::list_keys::KeyInfo;
use parsec_interface::operations::list_providers::{ProviderInfo, Uuid};
use parsec_interface::operations::psa_algorithm::*;
use parsec_interface::operations::psa_key_attributes::*;
use parsec_interface::operations::Convert;
use parsec_interface::operations::{NativeOperation, NativeResult};
use parsec_interface::operations_protobuf::ProtobufConverter;
use parsec_interface::requests::ProviderId;
use parsec_interface::requests::Response;
use parsec_interface::requests::ResponseStatus;
use parsec_interface::requests::{request::RequestHeader, Request};
use parsec_interface::requests::{AuthType, BodyType, Opcode};
use parsec_interface::secrecy::{ExposeSecret, Secret};
use std::collections::HashSet;
use std::io::ErrorKind;
use zeroize::Zeroizing;

const PROTOBUF_CONVERTER: ProtobufConverter = ProtobufConverter {};
const REQ_HEADER: RequestHeader = RequestHeader {
    provider: ProviderId::Core,
    session: 0,
    content_type: BodyType::Protobuf,
    accept_type: BodyType::Protobuf,
    auth_type: AuthType::NoAuth,
    opcode: Opcode::Ping,
};

fn get_response_bytes_from_result(result: NativeResult) -> Vec<u8> {
    let mut stream = MockStream::new();
    let mut req_hdr = REQ_HEADER;
    req_hdr.opcode = result.opcode();
    let mut resp = Response::from_request_header(req_hdr, ResponseStatus::Success);
    resp.body = PROTOBUF_CONVERTER.result_to_body(result).unwrap();
    resp.write_to_stream(&mut stream).unwrap();
    stream.pop_bytes_written()
}

fn get_req_from_bytes(bytes: Vec<u8>) -> Request {
    let mut stream = MockStream::new();
    stream.push_bytes_to_read(&bytes);
    Request::read_from_stream(&mut stream, usize::max_value()).unwrap()
}

fn get_operation_from_req_bytes(bytes: Vec<u8>) -> NativeOperation {
    let req = get_req_from_bytes(bytes);
    PROTOBUF_CONVERTER
        .body_to_operation(req.body, req.header.opcode)
        .unwrap()
}

#[test]
fn list_provider_test() {
    let mut client: TestBasicClient = Default::default();
    let provider_info = vec![ProviderInfo {
        uuid: Uuid::nil(),
        description: String::from("Some empty provider"),
        vendor: String::from("Arm Ltd."),
        version_maj: 1,
        version_min: 0,
        version_rev: 0,
        id: ProviderId::Core,
    }];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::ListProviders(operations::list_providers::Result {
            providers: provider_info,
        }),
    ));
    let providers = client.list_providers().expect("Failed to list providers");
    // Check request:
    // ListProviders request is empty so no checking to be done

    // Check response:
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].uuid, Uuid::nil());
}

#[test]
fn list_opcodes_test() {
    let mut client: TestBasicClient = Default::default();
    let mut opcodes = HashSet::new();
    let _ = opcodes.insert(Opcode::PsaDestroyKey);
    let _ = opcodes.insert(Opcode::PsaGenerateKey);
    client.set_mock_read(&get_response_bytes_from_result(NativeResult::ListOpcodes(
        operations::list_opcodes::Result { opcodes },
    )));
    let opcodes = client
        .list_opcodes(ProviderId::MbedCrypto)
        .expect("Failed to retrieve opcodes");
    // Check request:
    // ListOpcodes request is empty so no checking to be done

    // Check response:
    assert_eq!(opcodes.len(), 2);
    assert!(opcodes.contains(&Opcode::PsaGenerateKey) && opcodes.contains(&Opcode::PsaDestroyKey));
}

#[test]
fn list_clients_test() {
    let mut client: TestBasicClient = Default::default();
    let clients = vec!["toto".to_string(), "tata".to_string()];
    client.set_mock_read(&get_response_bytes_from_result(NativeResult::ListClients(
        operations::list_clients::Result { clients },
    )));
    let clients = client.list_clients().expect("Failed to retrieve opcodes");

    // Check response:
    assert_eq!(clients.len(), 2);
    assert!(clients.contains(&"toto".to_string()) && clients.contains(&"tata".to_string()));
}

#[test]
fn delete_client_test() {
    let mut client: TestBasicClient = Default::default();
    client.set_mock_read(&get_response_bytes_from_result(NativeResult::DeleteClient(
        operations::delete_client::Result {},
    )));
    let client_name = String::from("toto");
    client
        .delete_client(&client_name)
        .expect("Failed to call destroy key");

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::DeleteClient(op) = op {
        assert_eq!(op.client, client_name);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn list_keys_test() {
    use parsec_interface::operations::psa_key_attributes::{
        Attributes, Lifetime, Policy, Type, UsageFlags,
    };

    let mut client: TestBasicClient = Default::default();
    let mut usage_flags = UsageFlags::default();
    let _ = usage_flags
        .set_decrypt()
        .set_export()
        .set_copy()
        .set_cache()
        .set_encrypt()
        .set_decrypt()
        .set_sign_message()
        .set_verify_message()
        .set_sign_hash()
        .set_verify_hash()
        .set_derive();
    let key_info = vec![KeyInfo {
        provider_id: ProviderId::MbedCrypto,
        name: String::from("Foo"),
        attributes: Attributes {
            lifetime: Lifetime::Persistent,
            key_type: Type::RsaKeyPair,
            bits: 1024,
            policy: Policy {
                usage_flags,
                permitted_algorithms: Algorithm::AsymmetricSignature(
                    AsymmetricSignature::RsaPkcs1v15Sign {
                        hash_alg: Hash::Sha256.into(),
                    },
                ),
            },
        },
    }];

    client.set_mock_read(&get_response_bytes_from_result(NativeResult::ListKeys(
        operations::list_keys::Result { keys: key_info },
    )));

    let keys = client.list_keys().expect("Failed to list keys");
    // Check request:
    // ListKeys request is empty so no checking to be done

    // Check response:
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].name, "Foo");
    assert_eq!(keys[0].provider_id, ProviderId::MbedCrypto);
}

#[test]
fn core_provider_for_crypto_test() {
    let mut client: TestBasicClient = Default::default();

    client.set_implicit_provider(ProviderId::Core);
    let res = client
        .psa_destroy_key("random key")
        .expect_err("Expected a failure!!");

    assert!(matches!(
        res,
        Error::Client(ClientErrorKind::InvalidProvider)
    ));
}

#[test]
fn psa_generate_key_test() {
    let mut client: TestBasicClient = Default::default();
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaGenerateKey(operations::psa_generate_key::Result {}),
    ));
    let key_name = String::from("key-name");
    let mut usage_flags = UsageFlags::default();
    let _ = usage_flags
        .set_decrypt()
        .set_export()
        .set_copy()
        .set_cache()
        .set_decrypt();
    let key_attrs = Attributes {
        lifetime: Lifetime::Persistent,
        key_type: Type::Aes,
        bits: 192,
        policy: Policy {
            usage_flags,
            permitted_algorithms: Algorithm::Cipher(Cipher::Ctr),
        },
    };

    client
        .psa_generate_key(&key_name, key_attrs)
        .expect("failed to generate key");

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaGenerateKey(op) = op {
        assert_eq!(op.attributes, key_attrs);
        assert_eq!(op.key_name, key_name);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }

    // Check response:
    // GenerateKey response is empty so no checking to be done
}

#[test]
fn psa_destroy_key_test() {
    let mut client: TestBasicClient = Default::default();
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaDestroyKey(operations::psa_destroy_key::Result {}),
    ));
    let key_name = String::from("key-name");
    client
        .psa_destroy_key(&key_name)
        .expect("Failed to call destroy key");

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaDestroyKey(op) = op {
        assert_eq!(op.key_name, key_name);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }

    // Check response:
    // DestroyKey response is empty so no checking to be done
}

#[test]
fn psa_import_key_test() {
    let mut client: TestBasicClient = Default::default();
    client.set_mock_read(&get_response_bytes_from_result(NativeResult::PsaImportKey(
        operations::psa_import_key::Result {},
    )));
    let key_name = String::from("key-name");
    let mut usage_flags = UsageFlags::default();
    let _ = usage_flags
        .set_decrypt()
        .set_export()
        .set_copy()
        .set_cache()
        .set_decrypt();
    let key_attrs = Attributes {
        lifetime: Lifetime::Persistent,
        key_type: Type::Aes,
        bits: 192,
        policy: Policy {
            usage_flags,
            permitted_algorithms: Algorithm::Cipher(Cipher::Ctr),
        },
    };
    let key_data = vec![0xff_u8; 128];
    client
        .psa_import_key(&key_name, &key_data, key_attrs)
        .unwrap();

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaImportKey(op) = op {
        assert_eq!(op.attributes, key_attrs);
        assert_eq!(op.key_name, key_name);
        assert_eq!(op.data.expose_secret(), &key_data);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }

    // Check response:
    // ImportKey response is empty so no checking to be done
}

#[test]
fn psa_export_public_key_test() {
    let mut client: TestBasicClient = Default::default();
    let key_data = vec![0xa5; 128];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaExportPublicKey(operations::psa_export_public_key::Result {
            data: key_data.clone().into(),
        }),
    ));

    let key_name = String::from("key-name");
    // Check response:
    assert_eq!(
        client
            .psa_export_public_key(&key_name)
            .expect("Failed to export public key"),
        key_data
    );

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaExportPublicKey(op) = op {
        assert_eq!(op.key_name, key_name);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn psa_export_key_test() {
    let mut client: TestBasicClient = Default::default();
    let key_data = vec![0xa5; 128];
    client.set_mock_read(&get_response_bytes_from_result(NativeResult::PsaExportKey(
        operations::psa_export_key::Result {
            data: Secret::new(key_data.clone()),
        },
    )));

    let key_name = String::from("key-name");
    // Check response:
    assert_eq!(
        client
            .psa_export_key(&key_name)
            .expect("Failed to export key"),
        key_data
    );

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaExportKey(op) = op {
        assert_eq!(op.key_name, key_name);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn psa_sign_hash_test() {
    let mut client: TestBasicClient = Default::default();
    let hash = vec![0x77_u8; 32];
    let key_name = String::from("key_name");
    let sign_algorithm = AsymmetricSignature::Ecdsa {
        hash_alg: Hash::Sha256.into(),
    };
    let signature = vec![0x33_u8; 128];
    client.set_mock_read(&get_response_bytes_from_result(NativeResult::PsaSignHash(
        operations::psa_sign_hash::Result {
            signature: signature.clone().into(),
        },
    )));

    // Check response:
    assert_eq!(
        client
            .psa_sign_hash(&key_name, &hash, sign_algorithm)
            .expect("Failed to sign hash"),
        signature
    );

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaSignHash(op) = op {
        assert_eq!(op.key_name, key_name);
        assert_eq!(op.hash.to_vec(), hash);
        assert_eq!(op.alg, sign_algorithm);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn psa_generate_random_test() {
    let mut client: TestBasicClient = Default::default();
    let mock_result = vec![0xDE, 0xAD, 0xBE, 0xEF];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaGenerateRandom(operations::psa_generate_random::Result {
            random_bytes: Zeroizing::from(mock_result.clone()),
        }),
    ));

    // Check response:
    assert_eq!(
        client
            .psa_generate_random(4)
            .expect("failed to generate random"),
        mock_result
    );

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaGenerateRandom(op) = op {
        assert_eq!(op.size, 4);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn verify_hash_test() {
    let mut client: TestBasicClient = Default::default();
    let hash = vec![0x77_u8; 32];
    let key_name = String::from("key_name");
    let sign_algorithm = AsymmetricSignature::Ecdsa {
        hash_alg: Hash::Sha256.into(),
    };
    let signature = vec![0x33_u8; 128];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaVerifyHash(operations::psa_verify_hash::Result {}),
    ));

    client
        .psa_verify_hash(&key_name, &hash, sign_algorithm, &signature)
        .expect("Failed to sign hash");

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaVerifyHash(op) = op {
        assert_eq!(op.key_name, key_name);
        assert_eq!(op.hash.to_vec(), hash);
        assert_eq!(op.alg, sign_algorithm);
        assert_eq!(op.signature.to_vec(), signature);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }

    // Check response:
    // VerifyHash response is empty so no checking to be done
}

#[test]
fn psa_sign_message_test() {
    let mut client: TestBasicClient = Default::default();
    let msg = vec![0x77_u8; 100];
    let key_name = String::from("key_name");
    let sign_algorithm = AsymmetricSignature::Ecdsa {
        hash_alg: Hash::Sha256.into(),
    };
    let signature = vec![0x33_u8; 128];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaSignMessage(operations::psa_sign_message::Result {
            signature: signature.clone().into(),
        }),
    ));

    // Check response:
    assert_eq!(
        client
            .psa_sign_message(&key_name, &msg, sign_algorithm)
            .expect("Failed to sign message"),
        signature
    );

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaSignMessage(op) = op {
        assert_eq!(op.key_name, key_name);
        assert_eq!(op.message.to_vec(), msg);
        assert_eq!(op.alg, sign_algorithm);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn verify_message_test() {
    let mut client: TestBasicClient = Default::default();
    let msg = vec![0x77_u8; 100];
    let key_name = String::from("key_name");
    let sign_algorithm = AsymmetricSignature::Ecdsa {
        hash_alg: Hash::Sha256.into(),
    };
    let signature = vec![0x33_u8; 128];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaVerifyMessage(operations::psa_verify_message::Result {}),
    ));

    client
        .psa_verify_message(&key_name, &msg, sign_algorithm, &signature)
        .expect("Failed to sign hash");

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaVerifyMessage(op) = op {
        assert_eq!(op.key_name, key_name);
        assert_eq!(op.message.to_vec(), msg);
        assert_eq!(op.alg, sign_algorithm);
        assert_eq!(op.signature.to_vec(), signature);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }

    // Check response:
    // VerifyMessage response is empty so no checking to be done
}

#[test]
fn asymmetric_encrypt_test() {
    let mut client: TestBasicClient = Default::default();
    let plaintext = vec![0x77_u8; 32];
    let key_name = String::from("key_name");
    let encrypt_algorithm = AsymmetricEncryption::RsaPkcs1v15Crypt;
    let ciphertext = vec![0x33_u8; 128];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaAsymmetricEncrypt(operations::psa_asymmetric_encrypt::Result {
            ciphertext: ciphertext.clone().into(),
        }),
    ));

    // Check response:
    assert_eq!(
        client
            .psa_asymmetric_encrypt(&key_name, encrypt_algorithm, &plaintext, None)
            .expect("Failed to encrypt message"),
        ciphertext
    );

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaAsymmetricEncrypt(op) = op {
        assert_eq!(op.key_name, key_name);
        assert_eq!(op.alg, encrypt_algorithm);
        assert_eq!(*op.plaintext, plaintext);
        assert_eq!(op.salt, None);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn asymmetric_decrypt_test() {
    let mut client: TestBasicClient = Default::default();
    let plaintext = vec![0x77_u8; 32];
    let key_name = String::from("key_name");
    let encrypt_algorithm = AsymmetricEncryption::RsaPkcs1v15Crypt;
    let ciphertext = vec![0x33_u8; 128];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaAsymmetricDecrypt(operations::psa_asymmetric_decrypt::Result {
            plaintext: plaintext.clone().into(),
        }),
    ));

    // Check response
    assert_eq!(
        client
            .psa_asymmetric_decrypt(&key_name, encrypt_algorithm, &ciphertext, None)
            .expect("Failed to decrypt message"),
        plaintext
    );

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaAsymmetricDecrypt(op) = op {
        assert_eq!(op.key_name, key_name);
        assert_eq!(op.alg, encrypt_algorithm);
        assert_eq!(*op.ciphertext, ciphertext);
        assert_eq!(op.salt, None);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn aead_encrypt_test() {
    let mut client: TestBasicClient = Default::default();
    let plaintext = vec![0x77_u8; 32];
    let nonce = vec![0x0_u8; 32];
    let key_name = String::from("key_name");
    let encrypt_algorithm = Aead::AeadWithDefaultLengthTag(AeadWithDefaultLengthTag::Ccm);
    let ciphertext = vec![0x33_u8; 128];
    let additional_data = vec![0x55_u8; 128];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaAeadEncrypt(operations::psa_aead_encrypt::Result {
            ciphertext: ciphertext.clone().into(),
        }),
    ));

    // Check response
    assert_eq!(
        client
            .psa_aead_encrypt(
                &key_name,
                encrypt_algorithm,
                &nonce,
                &additional_data,
                &plaintext
            )
            .expect("Failed to encrypt message"),
        ciphertext
    );

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaAeadEncrypt(op) = op {
        assert_eq!(op.key_name, key_name);
        assert_eq!(op.alg, encrypt_algorithm);
        assert_eq!(*op.plaintext, plaintext);
        assert_eq!(*op.nonce, nonce);
        assert_eq!(*op.additional_data, additional_data);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn aead_decrypt_test() {
    let mut client: TestBasicClient = Default::default();
    let plaintext = vec![0x77_u8; 32];
    let nonce = vec![0x0_u8; 32];
    let key_name = String::from("key_name");
    let encrypt_algorithm = Aead::AeadWithDefaultLengthTag(AeadWithDefaultLengthTag::Ccm);
    let ciphertext = vec![0x33_u8; 128];
    let additional_data = vec![0x55_u8; 128];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaAeadDecrypt(operations::psa_aead_decrypt::Result {
            plaintext: plaintext.clone().into(),
        }),
    ));

    // Check response
    assert_eq!(
        client
            .psa_aead_decrypt(
                &key_name,
                encrypt_algorithm,
                &nonce,
                &additional_data,
                &ciphertext
            )
            .expect("Failed to decrypt message"),
        plaintext
    );

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaAeadDecrypt(op) = op {
        assert_eq!(op.key_name, key_name);
        assert_eq!(op.alg, encrypt_algorithm);
        assert_eq!(*op.ciphertext, ciphertext);
        assert_eq!(*op.nonce, nonce);
        assert_eq!(*op.additional_data, additional_data);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn hash_compute_test() {
    let mut client: TestBasicClient = Default::default();
    let message = vec![0x77_u8; 32];
    let hash_algorithm = Hash::Sha256;
    let hash = vec![0x33_u8; 128];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaHashCompute(operations::psa_hash_compute::Result {
            hash: hash.clone().into(),
        }),
    ));

    // Check response
    assert_eq!(
        client
            .psa_hash_compute(hash_algorithm, &message,)
            .expect("Failed to decrypt message"),
        hash
    );

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaHashCompute(op) = op {
        assert_eq!(op.alg, hash_algorithm);
        assert_eq!(*op.input, message);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn hash_compare_test() {
    let mut client: TestBasicClient = Default::default();
    let message = vec![0x77_u8; 32];
    let hash_algorithm = Hash::Sha256;
    let hash = vec![0x33_u8; 128];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaHashCompare(operations::psa_hash_compare::Result {}),
    ));

    // Check response
    client
        .psa_hash_compare(hash_algorithm, &message, &hash)
        .expect("Failed to decrypt message");

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaHashCompare(op) = op {
        assert_eq!(op.alg, hash_algorithm);
        assert_eq!(*op.input, message);
        assert_eq!(*op.hash, hash);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn mac_compute_test() {
    let mut client: TestBasicClient = Default::default();
    let key_name = "test";
    let message = vec![0x77_u8; 32];
    let mac_algorithm = Mac::FullLength( FullLengthMac::Hmac{ hash_alg: Hash::Sha256 } );
    let mac = vec![0x33_u8; 128];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaMacCompute(operations::psa_mac_compute::Result {
            mac: mac.clone().into(),
        }),
    ));

    // Check response
    assert_eq!(
        client
            .psa_mac_compute(mac_algorithm, &key_name, &message)
            .expect("Failed to decrypt message"),
        mac
    );

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaMacCompute(op) = op {
        assert_eq!(op.alg, mac_algorithm);
        assert_eq!(*op.input, message);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn mac_verify_test() {
    let mut client: TestBasicClient = Default::default();
    let key_name = "test";
    let message = vec![0x77_u8; 32];
    let mac_algorithm = Mac::FullLength( FullLengthMac::Hmac{ hash_alg: Hash::Sha256 } );
    let mac = vec![0x33_u8; 128];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaMacVerify(operations::psa_mac_verify::Result {}),
    ));

    // Check response
    client
        .psa_mac_verify(mac_algorithm, &key_name, &message, &mac)
        .expect("Failed to decrypt message");

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaMacVerify(op) = op {
        assert_eq!(op.alg, mac_algorithm);
        assert_eq!(*op.input, message);
        assert_eq!(*op.mac, mac);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn raw_key_agreement_test() {
    let mut client: TestBasicClient = Default::default();
    let key_name = String::from("key_name");
    let agreement_alg = RawKeyAgreement::Ecdh;
    let peer_key = vec![0x33_u8; 128];
    let shared_secret = vec![0xff_u8, 64];
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaRawKeyAgreement(operations::psa_raw_key_agreement::Result {
            shared_secret: Secret::new(shared_secret.clone()),
        }),
    ));

    // Check response
    assert_eq!(
        client
            .psa_raw_key_agreement(agreement_alg, &key_name, &peer_key)
            .expect("Failed key agreement"),
        shared_secret
    );

    // Check request:
    let op = get_operation_from_req_bytes(client.get_mock_write());
    if let NativeOperation::PsaRawKeyAgreement(op) = op {
        assert_eq!(op.private_key_name, key_name);
        assert_eq!(op.alg, agreement_alg);
        assert_eq!(*op.peer_key, peer_key);
    } else {
        panic!("Got wrong operation type: {:?}", op);
    }
}

#[test]
fn different_response_type_test() {
    let mut client: TestBasicClient = Default::default();
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaVerifyHash(operations::psa_verify_hash::Result {}),
    ));
    let key_name = String::from("key-name");
    let err = client
        .psa_destroy_key(&key_name)
        .expect_err("Error was expected");

    assert!(matches!(
        err,
        Error::Client(ClientErrorKind::InvalidServiceResponseType)
    ));
}

#[test]
fn response_status_test() {
    let mut client: TestBasicClient = Default::default();
    let mut stream = MockStream::new();
    let status = ResponseStatus::PsaErrorDataCorrupt;
    let mut resp = Response::from_request_header(REQ_HEADER, ResponseStatus::Success);
    resp.header.status = status;
    resp.write_to_stream(&mut stream).unwrap();
    client.set_mock_read(&stream.pop_bytes_written());
    let err = client.ping().expect_err("Error was expected");

    assert!(matches!(
        err,
        Error::Service(ResponseStatus::PsaErrorDataCorrupt)
    ));
}

#[test]
fn malformed_response_test() {
    let mut client: TestBasicClient = Default::default();
    client.set_mock_read(&[0xcb_u8; 130]);
    let err = client.ping().expect_err("Error was expected");

    assert!(matches!(
        err,
        Error::Client(ClientErrorKind::Interface(ResponseStatus::InvalidHeader))
    ));
}

#[test]
fn request_fields_test() {
    let mut client: TestBasicClient = Default::default();
    client.set_mock_read(&get_response_bytes_from_result(NativeResult::Ping(
        operations::ping::Result {
            wire_protocol_version_maj: 1,
            wire_protocol_version_min: 0,
        },
    )));
    let _ = client.ping().expect("Ping failed");

    let req = get_req_from_bytes(client.get_mock_write());
    assert_eq!(req.header, REQ_HEADER);
}

#[test]
fn auth_value_test() {
    let mut client: TestBasicClient = Default::default();
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaDestroyKey(operations::psa_destroy_key::Result {}),
    ));
    let key_name = String::from("key-name");
    client
        .psa_destroy_key(&key_name)
        .expect("Failed to call destroy key");

    let req = get_req_from_bytes(client.get_mock_write());
    assert_eq!(
        String::from_utf8(req.auth.buffer.expose_secret().to_owned()).unwrap(),
        String::from(DEFAULT_APP_NAME)
    );
}

#[test]
fn peer_credential_auth_test() {
    let mut client: TestBasicClient = Default::default();
    client.set_auth_data(Authentication::UnixPeerCredentials);
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::PsaDestroyKey(operations::psa_destroy_key::Result {}),
    ));
    let key_name = String::from("key-name");
    client
        .psa_destroy_key(&key_name)
        .expect("Failed to call destroy key");

    let req = get_req_from_bytes(client.get_mock_write());
    let current_uid: libc::uid_t = unsafe { libc::getuid() };
    assert_eq!(
        &current_uid.to_le_bytes().to_vec(),
        req.auth.buffer.expose_secret()
    );
}

#[test]
fn failing_ipc_test() {
    let mut client: TestBasicClient = Default::default();
    client.set_ipc_handler(Box::from(FailingMockIpc(FailingMockStream::new(
        ErrorKind::ConnectionRefused,
        "connection was refused, so rude",
        1,
    ))));

    let err = client.ping().expect_err("Expected to fail");
    assert!(matches!(
        err,
        Error::Client(ClientErrorKind::Interface(ResponseStatus::ConnectionError))
    ));
}

#[test]
fn set_default_auth_one_entry() {
    let mut client: TestBasicClient = Default::default();
    client.set_auth_data(Authentication::UnixPeerCredentials);
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::ListAuthenticators(operations::list_authenticators::Result {
            authenticators: vec![AuthenticatorInfo {
                description: String::new(),
                version_maj: 1,
                version_min: 0,
                version_rev: 0,
                id: AuthType::UnixPeerCredentials,
            }],
        }),
    ));

    client.set_default_auth(None).unwrap();
    assert_eq!(client.auth_data(), Authentication::UnixPeerCredentials);
}

#[test]
fn set_default_auth_three_entries() {
    let mut client: TestBasicClient = Default::default();
    client.set_auth_data(Authentication::UnixPeerCredentials);
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::ListAuthenticators(operations::list_authenticators::Result {
            authenticators: vec![
                AuthenticatorInfo {
                    description: String::new(),
                    version_maj: 1,
                    version_min: 0,
                    version_rev: 0,
                    id: AuthType::Jwt,
                },
                AuthenticatorInfo {
                    description: String::new(),
                    version_maj: 1,
                    version_min: 0,
                    version_rev: 0,
                    id: AuthType::NoAuth,
                },
                AuthenticatorInfo {
                    description: String::new(),
                    version_maj: 1,
                    version_min: 0,
                    version_rev: 0,
                    id: AuthType::UnixPeerCredentials,
                },
            ],
        }),
    ));

    client.set_default_auth(None).unwrap();
    assert_eq!(client.auth_data(), Authentication::UnixPeerCredentials);
}

#[test]
fn set_default_auth_direct() {
    let mut client: TestBasicClient = Default::default();
    client.set_auth_data(Authentication::UnixPeerCredentials);
    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::ListAuthenticators(operations::list_authenticators::Result {
            authenticators: vec![AuthenticatorInfo {
                description: String::new(),
                version_maj: 1,
                version_min: 0,
                version_rev: 0,
                id: AuthType::Direct,
            }],
        }),
    ));

    assert!(matches!(
        client.set_default_auth(None).unwrap_err(),
        Error::Client(ClientErrorKind::MissingParam)
    ));

    client.set_mock_read(&get_response_bytes_from_result(
        NativeResult::ListAuthenticators(operations::list_authenticators::Result {
            authenticators: vec![AuthenticatorInfo {
                description: String::new(),
                version_maj: 1,
                version_min: 0,
                version_rev: 0,
                id: AuthType::Direct,
            }],
        }),
    ));

    let app_name = String::from("some_app_name");
    client.set_default_auth(Some(app_name.clone())).unwrap();
    assert_eq!(client.auth_data(), Authentication::Direct(app_name));
}
