Demo release keystore for local/CI signed APKs.

Generate (already done in this environment when building):

```bash
keytool -genkeypair -v \
  -keystore solstice-release.jks \
  -alias solstice \
  -keyalg RSA -keysize 2048 -validity 10000 \
  -storepass solstice123 -keypass solstice123 \
  -dname "CN=Solstice Dispensary, OU=Mobile, O=Solstice, L=Portland, ST=OR, C=US"
```

The `.jks` file is gitignored. CI builds unsigned-or-debug unless the keystore is provided as a secret.
