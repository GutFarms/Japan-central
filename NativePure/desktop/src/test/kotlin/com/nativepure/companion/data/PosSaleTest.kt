package com.nativepure.companion.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.nio.file.Files

class PosSaleTest {
    @Before
    fun isolateStore() {
        val dir = Files.createTempDirectory("nativepure-pos-test").toFile()
        System.setProperty("nativepure.companion.dataDir", dir.absolutePath)
        System.setProperty("nativepure.companion.adminPassword", "12345678")
    }

    private fun signedInAdminRepo(): CompanionRepository {
        val repo = CompanionRepository()
        val login = repo.login("admin", "12345678")
        assertTrue("admin login failed: $login", login is AuthResult.Success)
        val profile = (login as AuthResult.Success).customer
        if (profile.mustChangePassword) {
            val changed = repo.forceChangePassword("AdminPass1!", "AdminPass1!")
            assertTrue(changed is AuthResult.Success)
        }
        return repo
    }

    @Test
    fun cashChangeMath() {
        val total = 27.50
        val tendered = 40.0
        val change = (tendered - total).coerceAtLeast(0.0)
        assertEquals(12.50, change, 0.001)
    }

    @Test
    fun completePosSale_rejectsShortCash() {
        val repo = signedInAdminRepo()
        val product = repo.catalogProducts().firstOrNull { it.stockQuantity > 0 && it.price > 0 }
            ?: repo.allProducts().first { it.stockQuantity > 0 }
        repo.clearCart()
        repo.addToCart(product.id, 1)
        val total = repo.cartSummary().total
        val result = repo.completePosSale(
            PosSaleRequest(
                customerName = "Test Walk-in",
                paymentMethod = PaymentMethod.CASH,
                amountTendered = (total - 1.0).coerceAtLeast(0.0),
                notes = "short cash"
            )
        )
        assertTrue(result is OpResult.Error)
    }

    @Test
    fun completePosSale_cardSucceedsAndDeductsStock() {
        val repo = signedInAdminRepo()
        val product = repo.catalogProducts().firstOrNull { it.stockQuantity > 0 && it.effectivePrice > 0 }
            ?: return
        val before = product.stockQuantity
        repo.clearCart()
        repo.addToCart(product.id, 1)
        val result = repo.completePosSale(
            PosSaleRequest(
                customerName = "Card Guest",
                paymentMethod = PaymentMethod.CARD,
                amountTendered = 0.0
            )
        )
        assertTrue("sale failed: $result", result is OpResult.Success)
        val after = repo.allProducts().first { it.id == product.id }.stockQuantity
        assertEquals(before - 1, after)
        val sale = repo.todaysPosSales().first()
        assertEquals(SaleChannel.POS.name, sale.channel)
        assertEquals(PaymentMethod.CARD.name, sale.paymentMethod)
        assertEquals(OrderStatus.PICKED_UP, sale.status)
    }
}
