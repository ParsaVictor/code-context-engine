function createStore(initial = {}) {
    const state = { ...initial };
    return {
        get(key) {
            return state[key];
        },
        set(key, value) {
            state[key] = value;
        },
        has(key) {
            return Object.prototype.hasOwnProperty.call(state, key);
        },
        keys() {
            return Object.keys(state);
        },
    };
}

module.exports = { createStore };
