package com.nativepure.companion.data

import org.junit.Assert.assertEquals
import org.junit.Test

class LoyaltyPointsTest {
    @Test
    fun pointsForSpend_floorsWholeDollars() {
        assertEquals(0, LoyaltyPoints.pointsForSpend(0.99))
        assertEquals(12, LoyaltyPoints.pointsForSpend(12.75))
    }

    @Test
    fun discountAndMaxRedeem() {
        assertEquals(1.0, LoyaltyPoints.discountForPoints(100), 0.001)
        assertEquals(200, LoyaltyPoints.maxRedeemablePoints(250, 40.0))
        assertEquals(0.8, LoyaltyPoints.taxOn(10.0, 0.08), 0.0001)
    }
}
