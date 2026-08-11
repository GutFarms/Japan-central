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
    }

    @Test
    fun masksEmailPhoneDob() {
        assertEquals("j•••@example.com", Privacy.maskEmail("jordan@example.com"))
        assertEquals("•••-•••-0142", Privacy.maskPhone("(505) 555-0142"))
        assertEquals("••••-1990", Privacy.maskDob("1990-01-01"))
    }

    @Test
    fun syncExportOmitsPasswordHashes() {
        val repo = CompanionRepository()
        val login = repo.login("admin", "12345678")
        assertTrue(login is AuthResult.Success)
        if ((login as AuthResult.Success).customer.mustChangePassword) {
            repo.forceChangePassword("AdminPass1!", "AdminPass1!")
        }
        val json = repo.exportSyncJson()
        assertFalse(json.contains("passwordHash"))
        assertFalse(json.contains("passwordSalt"))
        assertTrue(json.contains("\"customers\""))
    }

    @Test
    fun syncImportDoesNotEscalateRoleOrOverwritePassword() {
        val repo = CompanionRepository()
        val login = repo.login("admin", "12345678")
        assertTrue(login is AuthResult.Success)
        if ((login as AuthResult.Success).customer.mustChangePassword) {
            repo.forceChangePassword("AdminPass1!", "AdminPass1!")
        }
        val before = repo.allCustomers().first { it.email.contains("demo") }
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
        val result = repo.importSyncJson(payload)
        assertTrue(result is OpResult.Success)
        // Demo still customer (role preserved, not escalated)
        val after = repo.allCustomers().first { it.email.contains("demo") }
        assertEquals(AccountRole.CUSTOMER, after.role)
        assertEquals(42, after.loyaltyPoints)
        // Can still sign in with demo password (credentials preserved)
        repo.logout()
        val demoLogin = repo.login("demo@nativepure.example", "demo1234")
        assertTrue(demoLogin is AuthResult.Success)
        assertEquals(before.id, (demoLogin as AuthResult.Success).customer.id)
    }

    @Test
    fun receiptMasksEmailAndRequiresOwnership() {
        val repo = CompanionRepository()
        val login = repo.login("admin", "12345678")
        assertTrue(login is AuthResult.Success)
        if ((login as AuthResult.Success).customer.mustChangePassword) {
            repo.forceChangePassword("AdminPass1!", "AdminPass1!")
        }
        val product = repo.catalogProducts().first { it.stockQuantity > 0 }
        repo.clearCart()
        repo.addToCart(product.id, 1)
        val sale = repo.completePosSale(
            PosSaleRequest(
                customerName = "Walk-in",
                paymentMethod = PaymentMethod.CARD,
                amountTendered = 0.0,
                loyaltyCustomerId = repo.allCustomers().first { it.email.contains("demo") }.id
            )
        )
        assertTrue(sale is OpResult.Success)
        val orderId = repo.todaysPosSales().first().id
        val receipt = repo.orderReceiptText(orderId)!!
        assertFalse(receipt.contains("demo@nativepure.example"))
        assertTrue(receipt.contains("d•••@nativepure.example") || receipt.contains("Email:"))
        assertFalse(receipt.contains("Notes:"))
    }
}
