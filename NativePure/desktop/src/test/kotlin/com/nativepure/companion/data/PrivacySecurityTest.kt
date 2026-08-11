package com.nativepure.companion.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.nio.file.Files

class PrivacySecurityTest {
    @Before
    fun isolateStore() {
        val dir = Files.createTempDirectory("nativepure-privacy-test").toFile()
        System.setProperty("nativepure.companion.dataDir", dir.absolutePath)
        System.setProperty("nativepure.companion.adminPassword", "12345678")
    }

    private fun signedInAdmin(): CompanionRepository {
        val repo = CompanionRepository()
        val login = repo.login("admin", "12345678")
        assertTrue("admin login failed: $login", login is AuthResult.Success)
        if ((login as AuthResult.Success).customer.mustChangePassword) {
            assertTrue(repo.forceChangePassword("AdminPass1!", "AdminPass1!") is AuthResult.Success)
        }
        return repo
    }

    @Test
    fun masksEmailPhoneDob() {
        assertEquals("j•••@example.com", Privacy.maskEmail("jordan@example.com"))
        assertEquals("•••-•••-0142", Privacy.maskPhone("(505) 555-0142"))
        assertEquals("••••-1990", Privacy.maskDob("1990-01-01"))
    }

    @Test
    fun pbkdf2HashAndLegacyMatch() {
        val salt = PasswordHasher.newSalt()
        val hash = PasswordHasher.hash("Secret123", salt)
        assertTrue(hash.startsWith("pbkdf2$"))
        assertTrue(PasswordHasher.matches("Secret123", salt, hash))
        assertFalse(PasswordHasher.needsRehash(hash))
    }

    @Test
    fun storeIsEncryptedOnDisk() {
        signedInAdmin()
        val enc = LocalDataStore.encryptedStoreFile()
        assertTrue(enc.isFile)
        val text = enc.readText()
        assertTrue(text.startsWith("NPENC1:"))
        assertFalse(text.contains("passwordHash"))
        assertFalse(LocalDataStore.storeFile().exists())
    }

    @Test
    fun syncExportOmitsPasswordHashes() {
        val repo = signedInAdmin()
        val json = repo.exportSyncJson()
        assertFalse(json.contains("passwordHash"))
        assertFalse(json.contains("passwordSalt"))
    }

    @Test
    fun syncImportDoesNotEscalateRoleOrOverwritePassword() {
        val repo = signedInAdmin()
        val payload = """
            {
              "format": "nativepure-sync-v1",
              "exportedAt": 1,
              "products": [],
              "customers": [{
                "id": "cust-demo",
                "email": "demo@nativepure.example",
                "fullName": "Demo Customer",
                "role": "ADMIN",
                "loyaltyPoints": 42,
                "passwordHash": "evil",
                "passwordSalt": "evil"
              }],
              "orders": [],
              "orderLines": []
            }
        """.trimIndent()
        assertTrue(repo.importSyncJson(payload) is OpResult.Success)
        val after = repo.allCustomers().first { it.email.contains("demo") }
        assertEquals(AccountRole.CUSTOMER, after.role)
        assertEquals(42, after.loyaltyPoints)
        repo.logout()
        assertTrue(repo.login("demo@nativepure.example", "demo1234") is AuthResult.Success)
    }

    @Test
    fun staffSessionNotPersistedAcrossRestart() {
        val dir = System.getProperty("nativepure.companion.dataDir")
        val repo = signedInAdmin()
        assertTrue(repo.currentCustomer()?.role?.canViewSensitiveInfo == true)
        // Simulate process restart
        val repo2 = CompanionRepository()
        assertTrue(repo2.currentCustomer() == null)
        // Same data dir still encrypted
        assertTrue(LocalDataStore.encryptedStoreFile().isFile)
        assertEquals(dir, System.getProperty("nativepure.companion.dataDir"))
    }

    @Test
    fun receiptMasksEmail() {
        val repo = signedInAdmin()
        val product = repo.catalogProducts().first { it.stockQuantity > 0 }
        repo.clearCart()
        repo.addToCart(product.id, 1)
        assertTrue(
            repo.completePosSale(
                PosSaleRequest(
                    customerName = "Walk-in",
                    paymentMethod = PaymentMethod.CARD,
                    amountTendered = 0.0,
                    loyaltyCustomerId = repo.allCustomers().first { it.email.contains("demo") }.id
                )
            ) is OpResult.Success
        )
        val receipt = repo.orderReceiptText(repo.todaysPosSales().first().id)!!
        assertFalse(receipt.contains("demo@nativepure.example"))
        assertFalse(receipt.contains("Notes:"))
    }
}
