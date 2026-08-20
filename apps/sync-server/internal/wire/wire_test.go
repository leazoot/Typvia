package wire

import (
	"bytes"
	"crypto/ed25519"
	"crypto/rand"
	"crypto/sha256"
	"encoding/binary"
	"errors"
	"testing"
)

func testKey(t *testing.T) (ed25519.PublicKey, ed25519.PrivateKey) {
	t.Helper()
	pub, priv, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatalf("generate key: %v", err)
	}
	return pub, priv
}

func le64(v uint64) []byte {
	var b [8]byte
	binary.LittleEndian.PutUint64(b[:], v)
	return b[:]
}

func TestRootSignedBytesLayoutMatchesSpec(t *testing.T) {
	t.Parallel()
	ed := bytes.Repeat([]byte{0x11}, 32)
	x := bytes.Repeat([]byte{0x22}, 32)
	got, err := RootSignedBytes(RootSubject{
		DeviceID:   "FAKE_DEV",
		Ed25519Pub: ed,
		X25519Pub:  x,
		Name:       "FAKE_NAME",
		Platform:   "macos",
		CreatedAt:  1234,
	})
	if err != nil {
		t.Fatalf("root signed bytes: %v", err)
	}
	want := []byte("typvia.root.v1")
	want = append(want, []byte("FAKE_DEV\x00")...)
	want = append(want, ed...)
	want = append(want, x...)
	want = append(want, []byte("FAKE_NAME\x00macos\x00")...)
	want = append(want, le64(1234)...)
	if !bytes.Equal(got, want) {
		t.Fatalf("root_signed_bytes layout mismatch\n got %x\nwant %x", got, want)
	}
}

func TestRootSignedBytesRejectsEmbeddedNul(t *testing.T) {
	t.Parallel()
	_, err := RootSignedBytes(RootSubject{
		DeviceID:   "FAKE\x00DEV",
		Ed25519Pub: make([]byte, 32),
		X25519Pub:  make([]byte, 32),
		Name:       "n",
		Platform:   "macos",
	})
	if !errors.Is(err, ErrEmbeddedNul) {
		t.Fatalf("want ErrEmbeddedNul, got %v", err)
	}
}

func TestRecordSignedBytesTombstoneAndContentForms(t *testing.T) {
	t.Parallel()
	deletedAt := int64(777)
	ct := []byte("FAKE_CIPHERTEXT")
	content, err := RecordSignedBytes(RecordFields{
		DeviceID: "d1", EntityType: "snippet", EntityID: "e1",
		Version: 3, UpdatedAt: 55, Ciphertext: ct,
	})
	if err != nil {
		t.Fatalf("content signed bytes: %v", err)
	}
	tomb, err := RecordSignedBytes(RecordFields{
		DeviceID: "d1", EntityType: "snippet", EntityID: "e1",
		Version: 4, DeletedAt: &deletedAt, UpdatedAt: 56,
	})
	if err != nil {
		t.Fatalf("tombstone signed bytes: %v", err)
	}

	ctDigest := sha256.Sum256(ct)
	wantContent := []byte("typvia.syncrec.v1d1\x00snippet\x00e1\x00")
	wantContent = append(wantContent, le64(3)...)
	wantContent = append(wantContent, le64(^uint64(0))...)
	wantContent = append(wantContent, le64(55)...)
	wantContent = append(wantContent, ctDigest[:]...)
	if !bytes.Equal(content, wantContent) {
		t.Fatalf("content signed_bytes mismatch\n got %x\nwant %x", content, wantContent)
	}

	emptyDigest := sha256.Sum256(nil)
	wantTomb := []byte("typvia.syncrec.v1d1\x00snippet\x00e1\x00")
	wantTomb = append(wantTomb, le64(4)...)
	wantTomb = append(wantTomb, le64(777)...)
	wantTomb = append(wantTomb, le64(56)...)
	wantTomb = append(wantTomb, emptyDigest[:]...)
	if !bytes.Equal(tomb, wantTomb) {
		t.Fatalf("tombstone signed_bytes mismatch\n got %x\nwant %x", tomb, wantTomb)
	}
}

func TestVerifyAcceptsValidAndRejectsTamperedSignature(t *testing.T) {
	t.Parallel()
	pub, priv := testKey(t)
	msg := AuthSignedBytes(bytes.Repeat([]byte{0x42}, 32), "FAKE_DEVICE")
	sig := ed25519.Sign(priv, msg)
	if err := Verify(pub, msg, sig); err != nil {
		t.Fatalf("valid signature rejected: %v", err)
	}
	bad := append([]byte(nil), sig...)
	bad[0] ^= 0x01
	if err := Verify(pub, msg, bad); err == nil {
		t.Fatal("tampered signature accepted")
	}
	if err := Verify(pub[:31], msg, sig); err == nil {
		t.Fatal("short public key accepted")
	}
	if err := Verify(pub, msg, sig[:63]); err == nil {
		t.Fatal("short signature accepted")
	}
}

func TestRevokeAndRootProofMessagesAreDomainSeparated(t *testing.T) {
	t.Parallel()
	challenge := bytes.Repeat([]byte{0x07}, 32)
	revoke := RevokeSignedBytes("FAKE_DEVICE", 99)
	proof := RootProofSignedBytes(challenge, "FAKE_ACCOUNT")
	if !bytes.HasPrefix(revoke, []byte("typvia.revoke.v1")) {
		t.Fatalf("revoke message missing domain prefix: %x", revoke)
	}
	wantRevoke := append([]byte("typvia.revoke.v1FAKE_DEVICE"), le64(99)...)
	if !bytes.Equal(revoke, wantRevoke) {
		t.Fatalf("revoke message mismatch\n got %x\nwant %x", revoke, wantRevoke)
	}
	wantProof := append([]byte("typvia.rootproof.v1"), challenge...)
	wantProof = append(wantProof, []byte("FAKE_ACCOUNT")...)
	if !bytes.Equal(proof, wantProof) {
		t.Fatalf("root proof message mismatch\n got %x\nwant %x", proof, wantProof)
	}
}
