mod stream;

use std::net::{UdpSocket, SocketAddr};
use iso13400_2::{*, request};
use iso14229_1::{Configuration as Iso14229Cfg, response::Response as Iso14229Response, TryFromWithCfg};

use stream::Stream;
use crate::DoIpError;
use super::{config::Configuration, context::{GatewayInfo, PL_TYPES}};

/// [`Version`] and [`Payload`]
type VersionPayload = (Version, Payload);
pub type RoutingActiveStatus = (ActiveCode, Option<u8>);

#[derive(Debug)]
pub struct DoIpClient {
    config: Configuration,
    server_udp_addr: SocketAddr,
    udp_socket: UdpSocket,
    stream: Stream,
    gateway_info: Option<GatewayInfo>,
}

impl DoIpClient {
    pub fn new(config: Configuration) -> Result<Self, DoIpError> {
        let ip = config.server_ip();
        let udp_socket = UdpSocket::bind(format!("{}:0", ip))
            .map_err(DoIpError::IoError)?;
        let stream = Stream::connect(ip, None)?;
        let server_udp_addr = format!("{}:{}", ip, UDP_SERVER_PORT)
            .parse::<SocketAddr>()
            .unwrap();

        Ok(Self {
            config,
            server_udp_addr,
            udp_socket,
            stream,
            gateway_info: None,
        })
    }

    pub fn vehicle_identifier(&mut self) -> Result<(), DoIpError> {
        let request = Message {
            version: self.gateway_version(),
            payload: Payload::ReqVehicleId(request::VehicleID)
        };

        let response = self.udp_send_recv(request)?;
        self.vehicle_id_response(response)
    }

    pub fn vehicle_with_eid(&mut self, eid: Eid) -> Result<(), DoIpError> {
        let request = Message {
            version: self.gateway_version(),
            payload: Payload::ReqVehicleWithEid(request::VehicleIDWithEID::new(eid)),
        };

        let response = self.udp_send_recv(request)?;
        self.vehicle_id_response(response)
    }

    pub fn vehicle_with_vin(&mut self, vin: &str) -> Result<(), DoIpError> {
        let payload = request::VehicleIDWithVIN::new(vin)
            .map_err(DoIpError::Iso13400Error)?;
        let request = Message {
            version: self.gateway_version(),
            payload: Payload::ReqVehicleWithVIN(payload)
        };

        let response = self.udp_send_recv(request)?;
        self.vehicle_id_response(response)
    }

    pub fn routing_active(
        &mut self,
        r#type: RoutingActiveType,
        user_def: Option<u32>,
    ) -> Result<RoutingActiveStatus, DoIpError> {
        let src_addr = self.config.address();
        let request = Message {
            version: self.gateway_version(),
            payload: Payload::ReqRoutingActive(
                request::RoutingActive::new(src_addr, r#type, user_def)
            )
        };
        let (ver, resp) = self.tcp_write_read(request)?;
        match resp {
            Payload::RespHeaderNegative(v) => Err(self.on_header_negative_error(v)),
            Payload::RespRoutingActive(v) => {
                let dst_addr = v.dst_addr();
                if src_addr != dst_addr {
                    log::warn!("DoIPClient - routing active receive a message that target address: {:?}", dst_addr);
                }

                let active_code = v.active_code();
                match active_code {
                    ActiveCode::Activated |
                    ActiveCode::Success |
                    ActiveCode::NeedConfirm => Ok((active_code, None)),
                    ActiveCode::SourceAddressUnknown |
                    ActiveCode::SourceAddressInvalid |
                    ActiveCode::SocketInvalid |
                    ActiveCode::WithoutAuth |
                    ActiveCode::VehicleRefused |
                    ActiveCode::Unsupported |
                    ActiveCode::TLSRequired => Err(DoIpError::ActiveError(active_code)),
                    ActiveCode::VMSpecific(v) => {
                        log::info!("DoIPClient - routing active receive VM Specific value: {}", v);
                        Ok((active_code, Some(v)))
                    },
                    ActiveCode::Reserved(v) => {
                        log::warn!("DoIPClient - routing active receive Reserved value: {}", v);
                        Ok((active_code, Some(v)))
                    },
                }
            },
            _ => Err(DoIpError::UnexpectedResponse { version: ver, payload: resp }),
        }
    }

    pub fn alive_check(&mut self) -> Result<(), DoIpError> {
        let request = Message {
            version: self.gateway_version(),
            payload: Payload::ReqAliveCheck(request::AliveCheck)
        };
        let (ver, resp) = self.tcp_write_read(request)?;
        match resp {
            Payload::RespHeaderNegative(v) => Err(self.on_header_negative_error(v)),
            Payload::RespAliveCheck(v) => {
                log::info!("DoIPClient - alive check: {:?}", v.src_addr());
                Ok(())
            },
            _ => Err(DoIpError::UnexpectedResponse { version: ver, payload: resp }),
        }
    }

    pub fn entity_status(&mut self) -> Result<response::EntityStatus, DoIpError> {
        let request = Message {
            version: self.gateway_version(),
            payload: Payload::ReqEntityStatus(request::EntityStatus)
        };
        let (ver, resp) = self.udp_send_recv(request)?;
        match resp {
            Payload::RespHeaderNegative(v) => Err(self.on_header_negative_error(v)),
            Payload::RespEntityStatus(v) => Ok(v),
            _ => Err(DoIpError::UnexpectedResponse { version: ver, payload: resp }),
        }
    }

    pub fn diag_power_mode(&mut self) -> Result<PowerMode, DoIpError> {
        let request = Message {
            version: self.gateway_version(),
            payload: Payload::ReqDiagPowerMode(request::DiagnosticPowerMode)
        };
        let (ver, resp) = self.udp_send_recv(request)?;
        match resp {
            Payload::RespHeaderNegative(v) => Err(self.on_header_negative_error(v)),
            Payload::RespDiagPowerMode(v) => {
                Ok(v.mode())
            },
            _ => Err(DoIpError::UnexpectedResponse { version: ver, payload: resp }),
        }
    }

    pub fn diagnostic(
        &mut self,
        address: LogicAddress,
        data: Vec<u8>,
    ) -> Result<Iso14229Response, DoIpError> {
        let request = Message {
            version: self.gateway_version(),
            payload: Payload::Diagnostic(
                Diagnostic::new(self.config.address(), address, data)
            )
        };
        let (ver, resp) = self.tcp_write_read(request)?;
        match resp {
            Payload::RespHeaderNegative(v) => Err(self.on_header_negative_error(v)),
            Payload::RespDiagNegative(v) => {
                log::warn!("DoIPClient - {}", v);
                Err(DoIpError::DiagnosticNegativeError {
                    code: v.code(),
                    data: hex::encode(v.previous_diagnostic_data())
                })
            },
            Payload::RespDiagPositive(v) => {
                log::debug!("DoIPClient - Diagnostic message ACK: {}", v);
                let (ver, payload) = self.tcp_read(&PL_TYPES.diag_data_payload_types)?;
                match payload {
                    Payload::Diagnostic(v) => {
                        let data = v.data;
                        log::debug!("DoIPClient - diagnostic Data: {:?}", hex::encode(&data));
                        let cfg = Iso14229Cfg::default();
                        let resp = Iso14229Response::try_from_cfg(data, &cfg)
                            .map_err(DoIpError::Iso14229Error)?;

                        Ok(resp)
                    },
                    _ => Err(DoIpError::UnexpectedResponse { version: ver, payload }),
                }
            },
            _ => Err(DoIpError::UnexpectedResponse { version: ver, payload: resp }),
        }
    }

    #[inline]
    pub fn reconnect(&mut self) -> Result<(), DoIpError> {
        self.stream.reconnect(self.config.server_ip(), None)
    }

    #[inline]
    fn vehicle_id_response(&mut self, (ver, resp): VersionPayload) -> Result<(), DoIpError> {
        match resp {
            Payload::RespHeaderNegative(v) => Err(self.on_header_negative_error(v)),
            Payload::RespVehicleId(v) => {
                self.gateway_info = Some(GatewayInfo {
                    version: ver,
                    address: v.address(),
                    eid: v.eid(),
                    gid: v.gid(),
                    further_act: v.further_act(),
                    sync_status: v.sync_status(),
                });

                Ok(())
            },
            _ => Err(DoIpError::UnexpectedResponse { version: ver, payload: resp }),
        }
    }

    fn udp_send_recv(&mut self, request: Message) -> Result<VersionPayload, DoIpError> {
        let payload_type = request.payload.payload_type();
        let expect = match payload_type {
            PayloadType::ReqVehicleId => Some(&PL_TYPES.vid_payload_types),
            PayloadType::ReqVehicleWithEid => Some(&PL_TYPES.vid_payload_types),
            PayloadType::ReqVehicleWithVIN => Some(&PL_TYPES.vid_payload_types),
            PayloadType::ReqEntityStatus => Some(&PL_TYPES.es_payload_types),
            PayloadType::ReqDiagPowerMode => Some(&PL_TYPES.dpm_payload_types),
            _ => None
        }
            .ok_or(DoIpError::InputError(format!("invalid udp request payload: {:?}", payload_type)))?;
        let data: Vec<_> = request.into();
        log::trace!("DoIPClient - UDP writing data: {}", hex::encode(&data));
        let size = self.udp_socket.send_to(&data, &self.server_udp_addr)
            .map_err(DoIpError::IoError)?;
        let data_len = data.len();
        if size != data_len {
            log::warn!("DoIPClient - UDP wrote {} bytes, expect {}", size, data_len);
        }

        let mut buffer = [0; 1024];
        let size = self.udp_socket.recv(&mut buffer)
            .map_err(DoIpError::IoError)?;

        self.parse_response(&buffer[..size], expect)
    }

    fn tcp_write_read(&mut self, request: Message) -> Result<VersionPayload, DoIpError> {
        let payload_type = request.payload.payload_type();
        let expect = match payload_type {
            PayloadType::ReqRoutingActive => Some(&PL_TYPES.ra_payload_types),
            PayloadType::ReqAliveCheck => Some(&PL_TYPES.ac_payload_types),
            PayloadType::Diagnostic => Some(&PL_TYPES.diag_payload_types),
            _ => None,
        }
            .ok_or(DoIpError::InputError(format!("invalid udp request payload: {:?}", payload_type)))?;
        let data: Vec<_> = request.into();
        log::trace!("DoIPClient - TCP writing data: {}", hex::encode(&data));
        let size = self.stream.write(&data)?;
        let data_len = data.len();
        if size != data_len {
            log::warn!("DoIPClient - TCP wrote {} bytes, expect {}", size, data_len);
            Err(DoIpError::IoError(std::io::Error::last_os_error()))
        }
        else {
            self.tcp_read(expect)
        }
    }

    #[inline]
    fn tcp_read(&mut self, expected: &Vec<PayloadType>) -> Result<VersionPayload, DoIpError> {
        let mut buffer = [0; 4096];
        let size = self.stream.read(&mut buffer)?;

        self.parse_response(&buffer[..size], expected)
    }

    #[inline]
    fn parse_response(
        &mut self,
        data: &[u8],
        expected: &Vec<PayloadType>,
    ) -> Result<VersionPayload, DoIpError> {
        let response = Message::try_from(data)
            .map_err(DoIpError::Iso13400Error)?;
        let version = response.version;
        self.version_check(version);

        let actual = response.payload.payload_type();
        if expected.contains(&actual) {
            Ok((version, response.payload))
        }
        else {
            Err(DoIpError::PayloadTypeError(actual))
        }
    }

    fn version_check(&self, version: Version) {
        match &self.gateway_info {
            Some(info) => if info.version() != version {
                log::warn!("DoIPClient - DoIP version mismatch!");
            },
            None => {},
        }
    }

    #[inline]
    fn gateway_version(&self) -> Version {
        match &self.gateway_info {
            Some(info) => info.version,
            None => Version::Default,
        }
    }

    #[inline]
    fn on_header_negative_error(&mut self, header: response::HeaderNegative) -> DoIpError {
        let code = header.code();
        match code {
            HeaderNegativeCode::IncorrectPatternFormat |
            HeaderNegativeCode::InvalidPayloadLength => {
                self.stream
                    .shutdown()
                    .unwrap_or_else(|e| log::warn!("Error: {} when shutdown", e));
            },
            _ => {},
        }
        DoIpError::HeaderNegativeError(code)
    }
}
