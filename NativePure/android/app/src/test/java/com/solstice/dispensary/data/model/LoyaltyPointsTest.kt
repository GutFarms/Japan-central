package com.solstice.dispensary.data.model

import org.junit.Assert.assertEquals
import org.junit.Test

class LoyaltyPointsTest {
    @Test
    fun pointsForSpend_floorsWholeDollars() {
        assertEquals(0, LoyaltyPoints.pointsForSpend(0.0))
        assertEquals(0, LoyaltyPoints.pointsForSpend(0.99))
        assertEquals(12, LoyaltyPoints.pointsForSpend(12.75))
        assertEquals(0, LoyaltyPoints.pointsForSpend(-5.0))
    }

    @Test
    fun discountForPoints_isOneDollarPerHundred() {
        assertEquals(0.0, LoyaltyPoints.discountForPoints(0), 0.001)
        assertEquals(0.0, LoyaltyPoints.discountForPoints(99), 0.001)
        assertEquals(1.0, LoyaltyPoints.discountForPoints(100), 0.001)
        assertEquals(2.0, LoyaltyPoints.discountForPoints(250), 0.001)
    }

    @Test
    fun maxRedeemablePoints_respectsBalanceAndTotal() {
        assertEquals(0, LoyaltyPoints.maxRedeemablePoints(50, 40.0))
        assertEquals(200, LoyaltyPoints.maxRedeemablePoints(250, 40.0))
        assertEquals(100, LoyaltyPoints.maxRedeemablePoints(500, 1.5))
        assertEquals(0, LoyaltyPoints.maxRedeemablePoints(500, 0.0))
    }

    @Test
    fun taxOn_appliesRate() {
        assertEquals(0.8, LoyaltyPoints.taxOn(10.0, 0.08), 0.0001)
        assertEquals(0.0, LoyaltyPoints.taxOn(0.0), 0.0001)
    }
}
