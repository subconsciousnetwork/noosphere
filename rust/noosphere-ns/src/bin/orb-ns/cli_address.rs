//! Conversions between the metatype [CLIAddress] and
//! their destination types ([Url], [SocketAddr], [Multiaddr])
//! for both `clap` (as CLI flags) and `serde` (when parsing a config
//! file) to allow [SocketAddr], [IpAddr], TCP port, [Multiaddr], and
//! [Url] representations where appropriate.
use noosphere_ns::Multiaddr;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use url::Url;

use libp2p::multiaddr::Protocol;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Used in clap parsers, parses a string in [CLIAddress] form into
/// a [Multiaddr] or [SocketAddr].
pub fn parse_cli_address<T: TryFrom<CLIAddress>>(input: &str) -> Result<T, String> {
    let addr: CLIAddress = input
        .parse()
        .map_err(|_| String::from("invalid conversion"))?;

    addr.try_into()
        .map_err(|_| String::from("invalid conversion"))
}

/// Parses a string in [CLIAddress] form into a [SocketAddr] for
/// serde deserialization.
pub fn deserialize_socket_addr<'de, D>(deserializer: D) -> Result<Option<SocketAddr>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<CLIAddress>::deserialize(deserializer) {
        Ok(address) => match address {
            Some(addr) => match addr.try_into() {
                Ok(socket) => Ok(Some(socket)),
                Err(e) => Err(de::Error::custom(e.to_string())),
            },
            None => Ok(None),
        },
        Err(e) => Err(de::Error::custom(e.to_string())),
    }
}

/// Parses a string in [CLIAddress] form into a [Multiaddr] for
/// serde deserialization.
pub fn deserialize_multiaddr<'de, D>(deserializer: D) -> Result<Option<Multiaddr>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<CLIAddress>::deserialize(deserializer) {
        Ok(address) => match address {
            Some(addr) => match addr.try_into() {
                Ok(maddr) => Ok(Some(maddr)),
                Err(e) => Err(de::Error::custom(e.to_string())),
            },
            None => Ok(None),
        },
        Err(e) => Err(de::Error::custom(e.to_string())),
    }
}

#[derive(Debug, PartialEq)]
pub enum CLIAddress {
    Port(u16),
    Ip(IpAddr),
    Socket(SocketAddr),
    Url(Url),
    Multiaddr(Multiaddr),
}

impl Serialize for CLIAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            CLIAddress::Port(port) => {
                serializer.serialize_newtype_variant("CLIAddress", 0, "Port", &port)
            }
            CLIAddress::Ip(ip) => serializer.serialize_newtype_variant("CLIAddress", 1, "Ip", &ip),
            CLIAddress::Socket(socket) => {
                serializer.serialize_newtype_variant("CLIAddress", 2, "Socket", &socket)
            }
            CLIAddress::Url(ref url) => {
                serializer.serialize_newtype_variant("CLIAddress", 3, "Url", url)
            }
            CLIAddress::Multiaddr(ref addr) => {
                serializer.serialize_newtype_variant("CLIAddress", 4, "Multiaddr", addr)
            }
        }
    }
}

/// While we don't directly store structs with [CLIAddress] directly, as we want
/// to coerce to a "goal" type, like [Multiaddr] or [SocketAddr], the deserialization
/// is still used via `deserialize_multiaddr` and `deserialize_socket_addr`,
/// with some care to parse both integers (u16 ports) and strings.
impl<'de> Deserialize<'de> for CLIAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CLIAddressVisitor;
        impl<'de> de::Visitor<'de> for CLIAddressVisitor {
            type Value = CLIAddress;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    formatter,
                    "a string parseable as a u16 port, IpAddr, SocketAddr, Url, or Multiaddr."
                )
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if let Ok(addr) = s.parse() {
                    Ok(addr)
                } else {
                    Err(de::Error::custom("Could not parse as CLIAddress."))
                }
            }
            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value >= i64::from(u16::MIN) && value <= i64::from(u16::MAX) {
                    Ok(CLIAddress::Port(value as u16))
                } else {
                    Err(E::custom(format!("u16 out of range: {}", value)))
                }
            }
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value >= u64::from(u16::MIN) && value <= u64::from(u16::MAX) {
                    Ok(CLIAddress::Port(value as u16))
                } else {
                    Err(E::custom(format!("u16 out of range: {}", value)))
                }
            }
        }

        deserializer.deserialize_any(CLIAddressVisitor {})
    }
}

impl FromStr for CLIAddress {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(port) = <u16>::from_str(s) {
            return Ok(CLIAddress::Port(port));
        }
        if let Ok(ip) = <IpAddr>::from_str(s) {
            return Ok(CLIAddress::Ip(ip));
        }
        if let Ok(socket) = <SocketAddr>::from_str(s) {
            return Ok(CLIAddress::Socket(socket));
        }
        if let Ok(url) = <Url>::from_str(s) {
            return Ok(CLIAddress::Url(url));
        }
        if let Ok(maddr) = <Multiaddr>::from_str(s) {
            return Ok(CLIAddress::Multiaddr(maddr));
        }
        Err(anyhow::anyhow!("invalid conversion"))
    }
}

impl TryFrom<CLIAddress> for SocketAddr {
    type Error = anyhow::Error;

    fn try_from(value: CLIAddress) -> anyhow::Result<Self> {
        match value {
            CLIAddress::Port(port) => Ok(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                port,
            )),
            CLIAddress::Ip(addr) => Ok(SocketAddr::new(addr, 0)),
            CLIAddress::Socket(addr) => Ok(addr),
            _ => Err(anyhow::anyhow!("invalid conversion")),
        }
    }
}

impl TryFrom<CLIAddress> for Url {
    type Error = anyhow::Error;
    fn try_from(value: CLIAddress) -> anyhow::Result<Self> {
        match value {
            addr @ CLIAddress::Port(..)
            | addr @ CLIAddress::Ip(..)
            | addr @ CLIAddress::Socket(..) => {
                let socket = <CLIAddress as TryInto<SocketAddr>>::try_into(addr)?;
                let url = if socket.port() == 0 {
                    Url::parse(&format!("http://{}", socket.ip()))?
                } else {
                    Url::parse(&format!("http://{}:{}", socket.ip(), socket.port()))?
                };
                Ok(url)
            }
            CLIAddress::Url(url) => Ok(url),
            _ => Err(anyhow::anyhow!("invalid conversion")),
        }
    }
}

impl TryFrom<CLIAddress> for Multiaddr {
    type Error = anyhow::Error;
    fn try_from(value: CLIAddress) -> anyhow::Result<Self> {
        match value {
            addr @ CLIAddress::Port(..)
            | addr @ CLIAddress::Ip(..)
            | addr @ CLIAddress::Socket(..) => {
                let socket = <CLIAddress as TryInto<SocketAddr>>::try_into(addr)?;
                let mut multiaddr = Multiaddr::empty();
                Ok(match socket {
                    SocketAddr::V4(addr) => {
                        multiaddr.push(Protocol::Ip4(*addr.ip()));
                        multiaddr.push(Protocol::Tcp(addr.port()));
                        multiaddr
                    }
                    SocketAddr::V6(addr) => {
                        multiaddr.push(Protocol::Ip6(*addr.ip()));
                        multiaddr.push(Protocol::Tcp(addr.port()));
                        multiaddr
                    }
                })
            }
            CLIAddress::Multiaddr(addr) => Ok(addr),
            _ => Err(anyhow::anyhow!("invalid conversion")),
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use anyhow::Result;
    //use serde::Deserialize;

    #[test]
    fn cli_address_to_socket_addr() -> Result<()> {
        for (base, expectation) in vec![
            (CLIAddress::Port(1234), "127.0.0.1:1234"),
            (CLIAddress::Ip("0.0.0.0".parse()?), "0.0.0.0:0"),
            (
                CLIAddress::Socket("10.0.0.1:5555".parse()?),
                "10.0.0.1:5555",
            ),
        ] {
            let socket: SocketAddr = base.try_into()?;
            assert_eq!(&socket.to_string(), expectation);
        }

        for failure_addr in vec![
            CLIAddress::Url("http://127.0.0.1:6666".parse()?),
            CLIAddress::Multiaddr("/ip4/127.0.0.1/tcp/6666".parse()?),
        ] {
            let result: Result<SocketAddr> = failure_addr.try_into();
            assert!(result.is_err());
        }

        Ok(())
    }

    #[test]
    fn cli_address_to_multiaddr() -> Result<()> {
        for (base, expectation) in vec![
            (CLIAddress::Port(1234), "/ip4/127.0.0.1/tcp/1234"),
            (CLIAddress::Ip("0.0.0.0".parse()?), "/ip4/0.0.0.0/tcp/0"),
            (CLIAddress::Ip("::1".parse()?), "/ip6/::1/tcp/0"),
            (
                CLIAddress::Socket("10.0.0.1:1234".parse()?),
                "/ip4/10.0.0.1/tcp/1234",
            ),
            (
                CLIAddress::Socket("[::1]:1234".parse()?),
                "/ip6/::1/tcp/1234",
            ),
            (
                CLIAddress::Multiaddr("/ip4/10.0.0.1/tcp/1234".parse()?),
                "/ip4/10.0.0.1/tcp/1234",
            ),
        ] {
            let maddr: Multiaddr = base.try_into()?;
            assert_eq!(
                &maddr.to_string(),
                expectation,
                "expect {} to parse as {}",
                maddr,
                expectation
            );
        }

        for failure_addr in vec![CLIAddress::Url("http://127.0.0.1:6666".parse()?)] {
            let result: Result<Multiaddr> = failure_addr.try_into();
            assert!(result.is_err());
        }

        Ok(())
    }

    #[test]
    fn cli_address_to_url() -> Result<()> {
        for (base, expectation) in vec![
            (CLIAddress::Port(1234), "http://127.0.0.1:1234/"),
            (CLIAddress::Ip("0.0.0.0".parse()?), "http://0.0.0.0/"),
            (
                CLIAddress::Socket("10.0.0.1:5555".parse()?),
                "http://10.0.0.1:5555/",
            ),
            (
                CLIAddress::Url("http://10.0.0.1:5555".parse()?),
                "http://10.0.0.1:5555/",
            ),
        ] {
            let url: Url = base.try_into()?;
            assert_eq!(&url.to_string(), expectation);
        }

        for failure_addr in vec![CLIAddress::Multiaddr("/ip4/127.0.0.1/tcp/6666".parse()?)] {
            let result: Result<Url> = failure_addr.try_into();
            assert!(result.is_err());
        }

        Ok(())
    }

    #[test]
    fn test_parse_cli_address() -> Result<()> {
        assert_eq!(
            "/ip4/127.0.0.1/tcp/1234".parse::<Multiaddr>()?,
            parse_cli_address::<Multiaddr>("1234").unwrap(),
        );
        assert_eq!(
            "127.0.0.1:1234".parse::<SocketAddr>()?,
            parse_cli_address::<SocketAddr>("1234").unwrap(),
        );
        Ok(())
    }

    #[test]
    fn test_deserialize_cliaddress() -> Result<()> {
        #[derive(PartialEq, Debug, Deserialize)]
        pub struct TestDeserialize {
            pub port_str: CLIAddress,
            pub port_u16: CLIAddress,
            pub ip: CLIAddress,
            pub socket: CLIAddress,
            pub url: CLIAddress,
            pub multiaddr: CLIAddress,
        }

        assert_eq!(
            serde_json::from_str::<TestDeserialize>(
                r#"{
            "port_str": "1234",
            "port_u16": 25000, 
            "ip": "10.0.0.1",
            "socket": "10.0.0.1:1234",
            "url": "http://10.0.0.1:1234",
            "multiaddr": "/ip4/10.0.0.1/tcp/1234"
        }"#
            )?,
            TestDeserialize {
                port_str: CLIAddress::Port(1234),
                port_u16: CLIAddress::Port(25000),
                ip: CLIAddress::Ip("10.0.0.1".parse().unwrap()),
                socket: CLIAddress::Socket("10.0.0.1:1234".parse().unwrap()),
                url: CLIAddress::Url("http://10.0.0.1:1234".parse().unwrap()),
                multiaddr: CLIAddress::Multiaddr("/ip4/10.0.0.1/tcp/1234".parse().unwrap()),
            },
        );
        Ok(())
    }

    #[test]
    fn test_deserialize_socket_addr() -> Result<()> {
        #[derive(Deserialize)]
        pub struct TestDeserialize {
            #[serde(default, deserialize_with = "deserialize_socket_addr")]
            pub addr: Option<SocketAddr>,
        }
        let obj: TestDeserialize = serde_json::from_str(
            r#"{
            "addr": "1234"
        }"#,
        )?;
        assert_eq!(obj.addr.unwrap(), "127.0.0.1:1234".parse::<SocketAddr>()?);
        Ok(())
    }

    #[test]
    fn test_deserialize_multiaddr() -> Result<()> {
        #[derive(Deserialize)]
        pub struct TestDeserialize {
            #[serde(default, deserialize_with = "deserialize_multiaddr")]
            pub addr: Option<Multiaddr>,
        }
        let obj: TestDeserialize = serde_json::from_str(
            r#"{
            "addr": "1234"
        }"#,
        )?;
        assert_eq!(
            obj.addr.unwrap(),
            "/ip4/127.0.0.1/tcp/1234".parse::<Multiaddr>()?
        );
        Ok(())
    }
}
