use crate::capability::{Capabilities, Capability};
use serde_json::json;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn it_can_cast_between_map_and_sequence() {
    let cap_foo = Capability::from(("example://foo", "ability/foo", &json!({})));
    let cap_bar_1 = Capability::from(("example://bar", "ability/bar", &json!({ "beep": 1 })));
    let cap_bar_2 = Capability::from(("example://bar", "ability/bar", &json!({ "boop": 1 })));

    let cap_sequence = vec![cap_bar_1.clone(), cap_bar_2.clone(), cap_foo];
    let cap_map = Capabilities::try_from(&json!({
        "example://bar": {
            "ability/bar": [{ "beep": 1 }, { "boop": 1 }]
        },
        "example://foo": { "ability/foo": [{}] },
    }))
    .unwrap();

    assert_eq!(
        &cap_map.iter().collect::<Vec<Capability>>(),
        &cap_sequence,
        "Capabilities map to sequence."
    );
    assert_eq!(
        &Capabilities::try_from(cap_sequence).unwrap(),
        &cap_map,
        "Capabilities sequence to map."
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn it_rejects_non_compliant_json() {
    let failure_cases = [
        (json!([]), "resources must be map"),
        (
            json!({
                "resource:foo": []
            }),
            "abilities must be map",
        ),
        (
            json!({"resource:foo": {}}),
            "resource must have at least one ability",
        ),
        (
            json!({"resource:foo": { "ability/read": {} }}),
            "caveats must be array",
        ),
        (
            json!({"resource:foo": { "ability/read": [1] }}),
            "caveat must be object",
        ),
    ];

    for (json_data, message) in failure_cases {
        assert!(Capabilities::try_from(&json_data).is_err(), "{message}");
    }

    assert!(Capabilities::try_from(&json!({
        "resource:foo": { "ability/read": [{}] }
    }))
    .is_ok());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn it_filters_out_empty_caveats_when_iterating() {
    let cap_map = Capabilities::try_from(&json!({
        "example://bar": { "ability/bar": [{}] },
        "example://foo": { "ability/foo": [] }
    }))
    .unwrap();

    assert_eq!(
        cap_map.iter().collect::<Vec<Capability>>(),
        vec![Capability::from((
            "example://bar",
            "ability/bar",
            &json!({})
        ))],
        "iter() filters out capabilities with empty caveats"
    );
}
