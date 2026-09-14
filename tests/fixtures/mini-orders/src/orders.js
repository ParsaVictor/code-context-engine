const { createInventory } = require("./inventory");
const { lineTotal, totalFor } = require("./pricing");
const { formatLine } = require("./format");

function createOrderService(prices, taxRate = 0) {
    const inventory = createInventory();

    // BUG: placing an order must decrement inventory for each line.
    function placeOrder(lines) {
        let subtotal = 0;
        const receipt = [];
        for (const { sku, quantity } of lines) {
            const unit = prices[sku];
            const total = lineTotal(unit, quantity);
            subtotal += total;
            receipt.push(formatLine(sku, quantity, total));
        }
        return { total: totalFor(subtotal, taxRate), receipt };
    }

    return { inventory, placeOrder };
}

module.exports = { createOrderService };
