quick-test:
	cargo test -- --nocapture

verbose-test:
	cargo test --features verbose-tests -- --nocapture

HEADER_PATH = "https://docs.oasis-open.org/pkcs11/pkcs11-base/v3.0/cs01/include/pkcs11-v3.0/"
HEADER_DIR = "pkcs11v3"
get-headers:
	mkdir -p $(HEADER_DIR)
	wget -q $(HEADER_PATH)/pkcs11.h -O $(HEADER_DIR)/pkcs11.h
	wget -q $(HEADER_PATH)/pkcs11f.h -O $(HEADER_DIR)/pkcs11f.h
	wget -q $(HEADER_PATH)/pkcs11t.h -O $(HEADER_DIR)/pkcs11t.h
