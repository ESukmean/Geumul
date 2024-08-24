#[repr(u8)]
#[warn(dead_code, never_)]
pub enum ICMPDestinationUnreachableCode {
    DestinationNetworkUnreachable = 0,
    DestinationHostUnreachable = 1,
    SourceRouteFailed = 5,
    DestinationHostUnknown = 7,
}

pub(in crate::manager) fn generate_icmp_no_route_to_host_reply(
    original: [u8; 28],
    code: ICMPDestinationUnreachableCode,
) -> [u8; 56] {
    let mut packet = [0u8; 56];

    // IP Header
    packet[0] = 0x45; // Version and IHL (IPv4, 20 bytes)
    packet[1] = 0x00; // Type of Service
    packet[2] = 0x00; // Total Length (2 bytes, will be calculated later)
    packet[3] = 56; //
    packet[4] = 0x00; // Identification (2 bytes)
    packet[5] = 0x00;
    packet[6] = 0x00; // Flags/Fragment Offset
    packet[7] = 0x00;
    packet[8] = 0x40; // TTL (64)
    packet[9] = 0x01; // Protocol (ICMP)
    packet[10] = 0x00; // Header Checksum (2 bytes, will be calculated later)
    packet[11] = 0x00;
    packet[12..16].copy_from_slice(&[172, 29, 0, 1]); // Source IP (Placeholder)
    packet[16..20].copy_from_slice(&[172, 29, 0, 2]); // Destination IP (Placeholder)

    // ICMP Header
    packet[20] = 0x03; // ICMP Type: Destination Unreachable
    packet[21] = code as u8; // ICMP Code: Host Unreachable
    packet[22] = 0x00; // Checksum (2 bytes, will be calculated later)
    packet[23] = 0x00;
    packet[24] = 0x00; // Unused (4 bytes)
    packet[25] = 0x00;
    packet[26] = 0x00;
    packet[27] = 0x00;

    // Original IP Header and first 8 bytes of the original data (Placeholder)
    packet[28..56].copy_from_slice(&original);

    // Calculate checksum for ICMP header and data (20: IPv4, 8: 실제 데이터)
    let checksum = compute_checksum(&packet[20..28 + 20 + 8]);
    packet[22] = (checksum >> 8) as u8;
    packet[23] = (checksum & 0xff) as u8;

    // Calculate checksum for IP header
    let ip_checksum = compute_checksum(&packet[0..20]);
    packet[10] = (ip_checksum >> 8) as u8;
    packet[11] = (ip_checksum & 0xff) as u8;

    packet
}

fn compute_checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;

    // Sum all 16-bit words
    for i in (0..data.len()).step_by(2) {
        let word = u16::from_be_bytes([data[i], data[i + 1]]);
        sum = sum.wrapping_add(u32::from(word));
    }

    // Fold 32-bit sum to 16 bits
    while (sum >> 16) > 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }

    !(sum as u16)
}
