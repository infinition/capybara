//! AES-128 et AES-256 en ECB, sans dependance externe.
//!
//! Utilise uniquement pour dechiffrer le code de boot Sonix. Implementation de
//! reference, non durcie contre les attaques par canal auxiliaire, ce qui est
//! sans consequence ici puisque la cle est fournie par l'utilisateur et que le
//! chiffre est deja en clair sur le disque.

use std::sync::OnceLock;

struct Tables {
    sbox: [u8; 256],
    inv_sbox: [u8; 256],
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut sbox = [0u8; 256];
        let (mut p, mut q) = (1u8, 1u8);
        loop {
            p = p ^ (p << 1) ^ if p & 0x80 != 0 { 0x1b } else { 0 };
            q ^= q << 1;
            q ^= q << 2;
            q ^= q << 4;
            if q & 0x80 != 0 {
                q ^= 0x09;
            }
            sbox[p as usize] =
                q ^ q.rotate_left(1) ^ q.rotate_left(2) ^ q.rotate_left(3) ^ q.rotate_left(4) ^ 0x63;
            if p == 1 {
                break;
            }
        }
        sbox[0] = 0x63;

        let mut inv_sbox = [0u8; 256];
        for i in 0..256 {
            inv_sbox[sbox[i] as usize] = i as u8;
        }
        Tables { sbox, inv_sbox }
    })
}

/// Multiplication dans GF(2^8) modulo le polynome AES 0x11b.
fn gmul(mut a: u8, mut b: u8) -> u8 {
    let mut r = 0u8;
    while b != 0 {
        if b & 1 != 0 {
            r ^= a;
        }
        let hi = a & 0x80;
        a <<= 1;
        if hi != 0 {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    r
}

/// Cle AES etendue, valable pour des cles de 16 ou 32 octets.
pub struct Aes {
    round_keys: Vec<[u8; 16]>,
    rounds: usize,
}

impl Aes {
    pub fn new(key: &[u8]) -> Self {
        assert!(key.len() == 16 || key.len() == 32, "AES: cle de 16 ou 32 octets");
        let t = tables();
        let nk = key.len() / 4;
        let rounds = nk + 6;
        let words = 4 * (rounds + 1);

        let mut w = vec![[0u8; 4]; words];
        for i in 0..nk {
            w[i].copy_from_slice(&key[i * 4..i * 4 + 4]);
        }

        let mut rcon = 1u8;
        for i in nk..words {
            let mut temp = w[i - 1];
            if i % nk == 0 {
                temp.rotate_left(1);
                for b in temp.iter_mut() {
                    *b = t.sbox[*b as usize];
                }
                temp[0] ^= rcon;
                rcon = gmul(rcon, 2);
            } else if nk > 6 && i % nk == 4 {
                for b in temp.iter_mut() {
                    *b = t.sbox[*b as usize];
                }
            }
            for j in 0..4 {
                w[i][j] = w[i - nk][j] ^ temp[j];
            }
        }

        let round_keys = (0..=rounds)
            .map(|r| {
                let mut rk = [0u8; 16];
                for c in 0..4 {
                    rk[c * 4..c * 4 + 4].copy_from_slice(&w[r * 4 + c]);
                }
                rk
            })
            .collect();

        Self { round_keys, rounds }
    }

    fn add_round_key(state: &mut [u8; 16], rk: &[u8; 16]) {
        for i in 0..16 {
            state[i] ^= rk[i];
        }
    }

    /// Chiffre un bloc de 16 octets.
    pub fn encrypt_block(&self, block: &[u8; 16]) -> [u8; 16] {
        let t = tables();
        let mut s = *block;
        Self::add_round_key(&mut s, &self.round_keys[0]);

        for r in 1..=self.rounds {
            for b in s.iter_mut() {
                *b = t.sbox[*b as usize];
            }
            // ShiftRows : l'etat est en colonnes, l'octet (ligne, colonne) est a 4*col + ligne
            let p = s;
            for c in 0..4 {
                for row in 0..4 {
                    s[c * 4 + row] = p[((c + row) % 4) * 4 + row];
                }
            }
            if r != self.rounds {
                let p = s;
                for c in 0..4 {
                    let col = &p[c * 4..c * 4 + 4];
                    s[c * 4] = gmul(col[0], 2) ^ gmul(col[1], 3) ^ col[2] ^ col[3];
                    s[c * 4 + 1] = col[0] ^ gmul(col[1], 2) ^ gmul(col[2], 3) ^ col[3];
                    s[c * 4 + 2] = col[0] ^ col[1] ^ gmul(col[2], 2) ^ gmul(col[3], 3);
                    s[c * 4 + 3] = gmul(col[0], 3) ^ col[1] ^ col[2] ^ gmul(col[3], 2);
                }
            }
            Self::add_round_key(&mut s, &self.round_keys[r]);
        }
        s
    }

    /// Dechiffre un bloc de 16 octets.
    pub fn decrypt_block(&self, block: &[u8; 16]) -> [u8; 16] {
        let t = tables();
        let mut s = *block;
        Self::add_round_key(&mut s, &self.round_keys[self.rounds]);

        for r in (1..=self.rounds).rev() {
            // InvShiftRows
            let p = s;
            for c in 0..4 {
                for row in 0..4 {
                    s[((c + row) % 4) * 4 + row] = p[c * 4 + row];
                }
            }
            for b in s.iter_mut() {
                *b = t.inv_sbox[*b as usize];
            }
            Self::add_round_key(&mut s, &self.round_keys[r - 1]);
            if r != 1 {
                let p = s;
                for c in 0..4 {
                    let col = &p[c * 4..c * 4 + 4];
                    s[c * 4] = gmul(col[0], 14) ^ gmul(col[1], 11) ^ gmul(col[2], 13) ^ gmul(col[3], 9);
                    s[c * 4 + 1] = gmul(col[0], 9) ^ gmul(col[1], 14) ^ gmul(col[2], 11) ^ gmul(col[3], 13);
                    s[c * 4 + 2] = gmul(col[0], 13) ^ gmul(col[1], 9) ^ gmul(col[2], 14) ^ gmul(col[3], 11);
                    s[c * 4 + 3] = gmul(col[0], 11) ^ gmul(col[1], 13) ^ gmul(col[2], 9) ^ gmul(col[3], 14);
                }
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap()).collect()
    }

    fn block(s: &str) -> [u8; 16] {
        hex(s).try_into().unwrap()
    }

    #[test]
    fn fips197_aes128() {
        let aes = Aes::new(&hex("000102030405060708090a0b0c0d0e0f"));
        let ct = aes.encrypt_block(&block("00112233445566778899aabbccddeeff"));
        assert_eq!(ct, block("69c4e0d86a7b0430d8cdb78070b4c55a"));
        assert_eq!(aes.decrypt_block(&ct), block("00112233445566778899aabbccddeeff"));
    }

    #[test]
    fn fips197_aes256() {
        let aes = Aes::new(&hex(
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        ));
        let ct = aes.encrypt_block(&block("00112233445566778899aabbccddeeff"));
        assert_eq!(ct, block("8ea2b7ca516745bfeafc49904b496089"));
        assert_eq!(aes.decrypt_block(&ct), block("00112233445566778899aabbccddeeff"));
    }

    /// Vecteur specifique a la derivation d'IV Sonix : cle nulle, deviceKey repetee.
    #[test]
    fn sonix_iv_derivation() {
        let aes = Aes::new(&[0u8; 16]);
        let iv = aes.encrypt_block(&block("deadbeefdeadbeefdeadbeefdeadbeef"));
        assert_eq!(iv, block("f3321c62ed192c3f56618d0d4b3869f7"));
    }
}
