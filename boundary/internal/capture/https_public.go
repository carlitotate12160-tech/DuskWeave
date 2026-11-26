package capture

import (
	"crypto/sha256"
	"crypto/x509"
	"encoding/hex"
	"encoding/pem"
	"errors"
	"io"
	"os"

	"duskweave/boundary/internal/frame"
)

const PublicRootVersion = "curl-cacert-2026-11-05"
const PublicRootSHA = "427c064216761a1e72a8b30f0d954d08cb6e4435a82091fb865afd20c8497285"
const PublicRootSize = 200116
const PublicRootCertCount = 118
const PublicRootPath = "/etc/duskweave/public-roots-2026-11-05.pem"

func readBoundedFixedFile(path string, exact int) ([]byte, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, errors.New("trust")
	}
	defer f.Close()
	info, err := f.Stat()
	if err != nil || !info.Mode().IsRegular() {
		return nil, errors.New("trust")
	}
	data, err := io.ReadAll(io.LimitReader(f, int64(exact)+1))
	if err != nil || len(data) != exact {
		return nil, errors.New("trust")
	}
	return data, nil
}

func PublicPool(stored []byte) (*x509.CertPool, error) {
	if len(stored) != PublicRootSize {
		return nil, errors.New("trust")
	}
	sum := sha256.Sum256(stored)
	if hex.EncodeToString(sum[:]) != PublicRootSHA {
		return nil, errors.New("trust")
	}
	pool := x509.NewCertPool()
	count := 0
	rest := stored
	for {
		var block *pem.Block
		block, rest = pem.Decode(rest)
		if block == nil {
			break
		}
		if block.Type != "CERTIFICATE" {
			return nil, errors.New("trust")
		}
		cert, err := x509.ParseCertificate(block.Bytes)
		if err != nil {
			return nil, errors.New("trust")
		}
		pool.AddCert(cert)
		count++
	}
	if count != PublicRootCertCount || pool.Equal(x509.NewCertPool()) {
		return nil, errors.New("trust")
	}
	return pool, nil
}

func LoadPublicPool() (*x509.CertPool, error) {
	stored, err := readBoundedFixedFile(PublicRootPath, PublicRootSize)
	if err != nil {
		return nil, errors.New("trust")
	}
	return PublicPool(stored)
}

func loadTrustPool(runProfile string) (*x509.CertPool, error) {
	switch runProfile {
	case frame.LabRunProfile:
		return LoadLabPool()
	case frame.PublicRunProfile:
		return LoadPublicPool()
	default:
		return nil, errors.New("trust")
	}
}
