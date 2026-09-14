const { createStore } = require("./store");

function createInventory() {
    const stock = createStore();

    function addItem(sku, quantity) {
        const current = stock.has(sku) ? stock.get(sku) : 0;
        stock.set(sku, current + quantity);
        return stock.get(sku);
    }

    // BUG: an unknown sku should read as zero stock, not undefined.
    function stockOf(sku) {
        return stock.get(sku);
    }

    // BUG: removing more than is in stock must throw, not go negative.
    function removeItem(sku, quantity) {
        const current = stock.has(sku) ? stock.get(sku) : 0;
        stock.set(sku, current - quantity);
        return stock.get(sku);
    }

    return { addItem, removeItem, stockOf };
}

module.exports = { createInventory };
