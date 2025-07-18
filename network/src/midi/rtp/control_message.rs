use anyhow::{anyhow, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};

// Common header for all AppleMIDI control messages
#[derive(Debug, Clone, PartialEq)]
pub struct AppleMidiHeader {
    pub command: [u8; 2],
    pub protocol_version: u16,
    pub initiator_token: u32,
    pub ssrc: u32,
}

impl AppleMidiHeader {
    const PROTOCOL_VERSION: u16 = 2;

    fn serialize(&self, buf: &mut BytesMut) {
        buf.put_u8(0xFF);
        buf.put_u8(0xFF);
        buf.put_slice(&self.command);
        buf.put_u32(self.protocol_version.into());
        buf.put_u32(self.initiator_token);
        buf.put_u32(self.ssrc);
    }

    fn parse(reader: &mut Bytes) -> Result<Self> {
        if reader.len() < 12 {
            // 2 magic bytes + 2 command bytes + 4 version + 4 token + 4 ssrc = 16 bytes. Let's start with a minimal check.
            return Err(anyhow!("AppleMIDI header too short"));
        }

        let magic0 = reader.get_u8();
        let magic1 = reader.get_u8();
        if magic0 != 0xFF || magic1 != 0xFF {
            return Err(anyhow!("Invalid AppleMIDI magic bytes"));
        }

        let command = [reader.get_u8(), reader.get_u8()];
        let protocol_version = reader.get_u32() as u16;
        let initiator_token = reader.get_u32();
        let ssrc = reader.get_u32();

        if protocol_version != Self::PROTOCOL_VERSION {
            return Err(anyhow!(
                "Unsupported AppleMIDI protocol version: {}",
                protocol_version
            ));
        }

        Ok(Self {
            command,
            protocol_version,
            initiator_token,
            ssrc,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Invitation {
    pub header: AppleMidiHeader,
    pub name: String,
}

impl Invitation {
    pub fn new(initiator_token: u32, ssrc: u32, name: String) -> Self {
        Self {
            header: AppleMidiHeader {
                command: *b"IN",
                protocol_version: AppleMidiHeader::PROTOCOL_VERSION,
                initiator_token,
                ssrc,
            },
            name,
        }
    }

    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(128); // Reasonable starting capacity
        self.header.serialize(&mut buf);
        buf.put_slice(self.name.as_bytes());
        buf.put_u8(0); // NULL-terminator
        buf.freeze()
    }

    pub fn parse(mut reader: Bytes) -> Result<Self> {
        let header = AppleMidiHeader::parse(&mut reader)?;
        if header.command != *b"IN" {
            return Err(anyhow!("Not an Invitation message"));
        }
        let name_bytes = reader.split_to(reader.len() - 1); // Exclude NULL-terminator
        let name = String::from_utf8(name_bytes.to_vec())?;
        reader.advance(1); // Consume NULL-terminator
        Ok(Self { header, name })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InvitationAccepted {
    pub header: AppleMidiHeader,
    pub name: String,
}

impl InvitationAccepted {
    pub fn new(initiator_token: u32, ssrc: u32, name: String) -> Self {
        Self {
            header: AppleMidiHeader {
                command: *b"OK",
                protocol_version: AppleMidiHeader::PROTOCOL_VERSION,
                initiator_token,
                ssrc,
            },
            name,
        }
    }

    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(128);
        self.header.serialize(&mut buf);
        buf.put_slice(self.name.as_bytes());
        buf.put_u8(0); // NULL-terminator
        buf.freeze()
    }

    pub fn parse(mut reader: Bytes) -> Result<Self> {
        let header = AppleMidiHeader::parse(&mut reader)?;
        if header.command != *b"OK" {
            return Err(anyhow!("Not an Invitation Accepted message"));
        }
        let name_bytes = reader.split_to(reader.len() - 1);
        let name = String::from_utf8(name_bytes.to_vec())?;
        reader.advance(1);
        Ok(Self { header, name })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InvitationRejected {
    pub header: AppleMidiHeader,
}

impl InvitationRejected {
    pub fn new(initiator_token: u32, ssrc: u32) -> Self {
        Self {
            header: AppleMidiHeader {
                command: *b"NO",
                protocol_version: AppleMidiHeader::PROTOCOL_VERSION,
                initiator_token,
                ssrc,
            },
        }
    }

    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(16);
        self.header.serialize(&mut buf);
        buf.freeze()
    }

    pub fn parse(mut reader: Bytes) -> Result<Self> {
        let header = AppleMidiHeader::parse(&mut reader)?;
        if header.command != *b"NO" {
            return Err(anyhow!("Not an Invitation Rejected message"));
        }
        Ok(Self { header })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Exit {
    pub header: AppleMidiHeader,
}

impl Exit {
    pub fn new(initiator_token: u32, ssrc: u32) -> Self {
        Self {
            header: AppleMidiHeader {
                command: *b"BY",
                protocol_version: AppleMidiHeader::PROTOCOL_VERSION,
                initiator_token,
                ssrc,
            },
        }
    }

    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(16);
        self.header.serialize(&mut buf);
        buf.freeze()
    }

    pub fn parse(mut reader: Bytes) -> Result<Self> {
        let header = AppleMidiHeader::parse(&mut reader)?;
        if header.command != *b"BY" {
            return Err(anyhow!("Not an Exit message"));
        }
        Ok(Self { header })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sync {
    pub ssrc: u32,
    pub count: u8,
    pub timestamps: [u64; 3],
}

impl Sync {
    pub fn new(ssrc: u32, count: u8, timestamps: [u64; 3]) -> Self {
        Self {
            ssrc,
            count,
            timestamps,
        }
    }

    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(32); // SSRC (4) + count (1) + padding (3) + timestamps (8*3 = 24)
        buf.put_u8(0xFF);
        buf.put_u8(0xFF);
        buf.put_slice(b"CK");
        buf.put_u32(self.ssrc);
        buf.put_u8(self.count);
        buf.put_u8(0); // Padding
        buf.put_u8(0); // Padding
        buf.put_u8(0); // Padding
        buf.put_u64(self.timestamps[0]);
        buf.put_u64(self.timestamps[1]);
        buf.put_u64(self.timestamps[2]);
        buf.freeze()
    }

    pub fn parse(mut reader: Bytes) -> Result<Self> {
        if reader.len() < 32 {
            return Err(anyhow!("Sync message too short"));
        }
        let magic0 = reader.get_u8();
        let magic1 = reader.get_u8();
        if magic0 != 0xFF || magic1 != 0xFF {
            return Err(anyhow!("Invalid AppleMIDI magic bytes"));
        }
        let command = [reader.get_u8(), reader.get_u8()];
        if command != *b"CK" {
            return Err(anyhow!("Not a Sync message"));
        }
        let ssrc = reader.get_u32();
        let count = reader.get_u8();
        reader.advance(3); // Skip padding
        let timestamps = [reader.get_u64(), reader.get_u64(), reader.get_u64()];
        Ok(Self {
            ssrc,
            count,
            timestamps,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReceiverFeedback {
    pub ssrc: u32,
    pub sequence_number: u16,
}

impl ReceiverFeedback {
    pub fn new(ssrc: u32, sequence_number: u16) -> Self {
        Self {
            ssrc,
            sequence_number,
        }
    }

    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(12); // SSRC (4) + sequence_number (2) + padding (2)
        buf.put_u8(0xFF);
        buf.put_u8(0xFF);
        buf.put_slice(b"RS");
        buf.put_u32(self.ssrc);
        buf.put_u16(self.sequence_number);
        buf.put_u16(0); // Padding
        buf.freeze()
    }

    pub fn parse(mut reader: Bytes) -> Result<Self> {
        if reader.len() < 12 {
            return Err(anyhow!("ReceiverFeedback message too short"));
        }
        let magic0 = reader.get_u8();
        let magic1 = reader.get_u8();
        if magic0 != 0xFF || magic1 != 0xFF {
            return Err(anyhow!("Invalid AppleMIDI magic bytes"));
        }
        let command = [reader.get_u8(), reader.get_u8()];
        if command != *b"RS" {
            return Err(anyhow!("Not a ReceiverFeedback message"));
        }
        let ssrc = reader.get_u32();
        let sequence_number = reader.get_u16();
        reader.advance(2); // Skip padding
        Ok(Self {
            ssrc,
            sequence_number,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppleMidiMessage {
    Invitation(Invitation),
    InvitationAccepted(InvitationAccepted),
    InvitationRejected(InvitationRejected),
    Exit(Exit),
    Sync(Sync),
    ReceiverFeedback(ReceiverFeedback),
}

impl AppleMidiMessage {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let reader = Bytes::copy_from_slice(data);
        if reader.len() < 4 {
            return Err(anyhow!("Message too short for command detection"));
        }
        let command_bytes = [reader.chunk()[2], reader.chunk()[3]];

        match &command_bytes {
            b"IN" => Ok(AppleMidiMessage::Invitation(Invitation::parse(reader)?)),
            b"OK" => Ok(AppleMidiMessage::InvitationAccepted(
                InvitationAccepted::parse(reader)?,
            )),
            b"NO" => Ok(AppleMidiMessage::InvitationRejected(
                InvitationRejected::parse(reader)?,
            )),
            b"BY" => Ok(AppleMidiMessage::Exit(Exit::parse(reader)?)),
            b"CK" => Ok(AppleMidiMessage::Sync(Sync::parse(reader)?)),
            b"RS" => Ok(AppleMidiMessage::ReceiverFeedback(ReceiverFeedback::parse(
                reader,
            )?)),
            _ => Err(anyhow!("Unknown AppleMIDI command: {:?}", command_bytes)),
        }
    }

    pub fn serialize(&self) -> Bytes {
        match self {
            AppleMidiMessage::Invitation(msg) => msg.serialize(),
            AppleMidiMessage::InvitationAccepted(msg) => msg.serialize(),
            AppleMidiMessage::InvitationRejected(msg) => msg.serialize(),
            AppleMidiMessage::Exit(msg) => msg.serialize(),
            AppleMidiMessage::Sync(msg) => msg.serialize(),
            AppleMidiMessage::ReceiverFeedback(msg) => msg.serialize(),
        }
    }
}
