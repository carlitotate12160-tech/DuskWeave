// Package main implements the synthetic offline HTTPS LAB responder binary.
package main

import (
	"crypto/ed25519"
	"crypto/rand"
	"crypto/sha256"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"errors"
	"math/big"
	"time"
)

const labHostname = "head.duskweave.test"

// LabRootPEM matches boundary/fixtures/https-lab-root-v1.pem (duskweave-https-lab-root-v1).
const LabRootPEM = "" +
	"-----BEGIN CERTIFICATE-----\n" +
	"MIIBMDCB46ADAgECAgEBMAUGAytlcDAuMSwwKgYDVQQDDCNEdXNrV2VhdmUgSFRU\n" +
	"UFMgTEFCIHN5bnRoZXRpYyBDQSB2MTAeFw0yNjAxMDEwMDAwMDBaFw0zNjAxMDEw\n" +
	"MDAwMDBaMC4xLDAqBgNVBAMMI0R1c2tXZWF2ZSBIVFRQUyBMQUIgc3ludGhldGlj\n" +
	"IENBIHYxMCowBQYDK2VwAyEA9S2rYOxlb05g+iGfpsz0lN4t3QstCmPeK6CvlbJx\n" +
	"6EajJjAkMBIGA1UdEwEB/wQIMAYBAf8CAQAwDgYDVR0PAQH/BAQDAgEGMAUGAytl\n" +
	"cANBAOhEstA3uNRD0FpTw+z2PqY888Nz7OuQq5cyIObzaeNARHQegUDx981yjloJ\n" +
	"yV2OaKbiFpepxxKR2ENft9CBUwc=\n" +
	"-----END CERTIFICATE-----\n"

func buildTLSConfig(scenario string) (*tls.Config, error) {
	if !IsValidScenario(scenario) {
		return nil, errors.New("invalid_scenario")
	}
	caCert, caKey, err := loadSyntheticRoot()
	if err != nil {
		return nil, err
	}
	if scenario == "wrong_root" {
		caCert, caKey, err = generateRogueRoot()
		if err != nil {
			return nil, err
		}
	}
	leaf, err := generateLeafCert(scenario, caCert, caKey)
	if err != nil {
		return nil, err
	}
	protos := []string{"http/1.1"}
	if scenario == "http2" {
		protos = []string{"h2"}
	}
	return &tls.Config{
		MinVersion:   tls.VersionTLS12,
		Certificates: []tls.Certificate{leaf},
		NextProtos:   protos,
	}, nil
}

func loadSyntheticRoot() (*x509.Certificate, ed25519.PrivateKey, error) {
	block, rest := pem.Decode([]byte(LabRootPEM))
	if block == nil || len(rest) != 0 || block.Type != "CERTIFICATE" {
		return nil, nil, errors.New("invalid_root_pem")
	}
	cert, err := x509.ParseCertificate(block.Bytes)
	if err != nil {
		return nil, nil, err
	}
	seed := sha256.Sum256([]byte("duskweave-https-lab-v1-public-synthetic-ca-key"))
	return cert, ed25519.NewKeyFromSeed(seed[:]), nil
}

func generateRogueRoot() (*x509.Certificate, ed25519.PrivateKey, error) {
	_, priv, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		return nil, nil, err
	}
	now := time.Now().UTC()
	tmpl := &x509.Certificate{
		SerialNumber:          big.NewInt(99),
		Subject:               pkix.Name{CommonName: "Rogue Synthetic CA"},
		NotBefore:             now.Add(-24 * time.Hour),
		NotAfter:              now.Add(24 * time.Hour),
		KeyUsage:              x509.KeyUsageCertSign | x509.KeyUsageCRLSign,
		BasicConstraintsValid: true,
		IsCA:                  true,
	}
	der, err := x509.CreateCertificate(rand.Reader, tmpl, tmpl, priv.Public(), priv)
	if err != nil {
		return nil, nil, err
	}
	cert, err := x509.ParseCertificate(der)
	if err != nil {
		return nil, nil, err
	}
	return cert, priv, nil
}

func generateLeafCert(scenario string, caCert *x509.Certificate, caKey ed25519.PrivateKey) (tls.Certificate, error) {
	_, leafPriv, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		return tls.Certificate{}, err
	}
	now := time.Now().UTC()
	host := labHostname
	notBefore, notAfter := now.Add(-time.Hour), now.Add(24*time.Hour)
	switch scenario {
	case "wrong_hostname":
		host = "wrong.duskweave.test"
	case "expired":
		notBefore = now.Add(-48 * time.Hour)
		notAfter = now.Add(-24 * time.Hour)
	}
	tmpl := &x509.Certificate{
		SerialNumber: big.NewInt(2),
		Subject:      pkix.Name{CommonName: host},
		DNSNames:     []string{host},
		NotBefore:    notBefore,
		NotAfter:     notAfter,
		KeyUsage:     x509.KeyUsageDigitalSignature,
		ExtKeyUsage:  []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
	}
	der, err := x509.CreateCertificate(rand.Reader, tmpl, caCert, leafPriv.Public(), caKey)
	if err != nil {
		return tls.Certificate{}, err
	}
	return tls.Certificate{
		Certificate: [][]byte{der},
		PrivateKey:  leafPriv,
	}, nil
}
