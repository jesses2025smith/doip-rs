use std::{io::{Read, Write}, net::{Shutdown, TcpStream}};
use iso13400_2::{TCP_SERVER_PORT, TLS_TCP_SERVER_PORT};
use native_tls::{Certificate, TlsConnector, TlsStream};
use crate::DoIpError;

#[derive(Debug)]
pub(crate) struct Stream {
    tls: bool,
    tcp_stream: Option<TcpStream>,
    tls_stream: Option<TlsStream<TcpStream>>,
}

impl Stream {
    pub fn connect(ip: &str, cert: Option<Certificate>) -> Result<Self, DoIpError> {
        let tls = cert.is_some();
        let (tcp_stream, tls_stream) = if tls {
            let tcp_stream = TcpStream::connect(format!("{}:{}", ip, TLS_TCP_SERVER_PORT))
                .map_err(DoIpError::IoError)?;

            let tls_stream = Self::tls_connector(cert)?
                .connect(ip, tcp_stream)
                .map_err(|e| DoIpError::TlsError(e.to_string()))?;

            (None, Some(tls_stream))
        }
        else {
            let tcp_stream = TcpStream::connect(format!("{}:{}", ip, TCP_SERVER_PORT))
                .map_err(DoIpError::IoError)?;
            (Some(tcp_stream), None)
        };

        Ok(Self { tls, tcp_stream, tls_stream })
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize, DoIpError> {
        unsafe {
            if self.tls {
                self.tcp_stream
                    .as_mut()
                    .unwrap_unchecked()
                    .write(data)
                    .map_err(DoIpError::IoError)
            }
            else {
                self.tcp_stream
                    .as_mut()
                    .unwrap_unchecked()
                    .write(data)
                    .map_err(DoIpError::IoError)
            }
        }
    }

    pub fn read(&mut self, data: &mut [u8]) -> Result<usize, DoIpError> {
        unsafe {
            if self.tls {
                self.tcp_stream
                    .as_mut()
                    .unwrap_unchecked()
                    .read(data)
                    .map_err(DoIpError::IoError)
            }
            else {
                self.tcp_stream
                    .as_mut()
                    .unwrap_unchecked()
                    .read(data)
                    .map_err(DoIpError::IoError)
            }
        }
    }

    pub fn reconnect(&mut self, ip: &str, cert: Option<Certificate>) -> Result<(), DoIpError> {
        self.tls = cert.is_some();
        if self.tls {
            let tcp_stream = TcpStream::connect(format!("{}:{}", ip, TLS_TCP_SERVER_PORT))
                .map_err(DoIpError::IoError)?;

            let tls_stream = Self::tls_connector(cert)?
                .connect(ip, tcp_stream)
                .map_err(|e| DoIpError::TlsError(e.to_string()))?;

            self.tls_stream = Some(tls_stream);
        }
        else {
            let tcp_stream = TcpStream::connect(format!("{}:{}", ip, TCP_SERVER_PORT))
                .map_err(DoIpError::IoError)?;
            self.tcp_stream = Some(tcp_stream);
        };

        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<(), DoIpError> {
        if let Some(tcp_stream) = self.tcp_stream.as_ref() {
            tcp_stream.shutdown(Shutdown::Both)
                .map_err(DoIpError::IoError)?;
        }

        if let Some(tls_stream) = self.tls_stream.as_mut() {
            tls_stream.shutdown()
                .map_err(DoIpError::IoError)?;
        }

        Ok(())
    }

    #[inline]
    fn tls_connector(cert: Option<Certificate>) -> Result<TlsConnector, DoIpError> {
        let mut tls_builder = TlsConnector::builder();
        #[cfg(not(feature = "develop"))]
        tls_builder.danger_accept_invalid_certs(false);
        #[cfg(feature = "develop")]
        tls_builder.danger_accept_invalid_certs(true);

        cert.map(|cert| tls_builder.add_root_certificate(cert));
        tls_builder.build()
            .map_err(|e| DoIpError::TlsError(e.to_string()))
    }
}
