// TASK-125: SPU2 ADSR envelope decay/release underflow fixture (Issue #104).
// Usage: spu2_envelope_fixture

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstdlib>

namespace {

void require(bool condition, const char *message) {
    if (!condition) {
        std::fprintf(stderr, "FAIL: %s\n", message);
        std::exit(1);
    }
}

// Struct matching derived runtime/src/iop/spu2.cpp
struct Envelope {
    int counter = 0;
    std::int32_t level = 0;

    void step(int rate, bool decrease, bool exponential) {
        const int shift = rate >> 2;
        int cycles = 1 << std::max(0, shift - 11);
        if (exponential && !decrease && level > 0x6000) cycles *= 4;
        if (++counter < cycles) return;
        counter = 0;
        int delta = (decrease ? -8 + (rate & 3) : 7 - (rate & 3)) << std::max(0, 11 - shift);
        if (exponential && decrease) {
            delta = delta * level / 0x8000;
            if (delta == 0 && level > 0) delta = -1;
        }
        level = std::clamp(level + delta, 0, 0x7FFF);
    }
};

// Test that exponential decrease reliably decays to 0 for all 32 ADSR release rates
void test_all_release_rates_decay_to_zero() {
    for (int r = 0; r < 32; ++r) {
        const int rate = r << 2;
        Envelope env;
        env.level = 0x7FFF;
        // Step until settled or maximum expected cycles
        for (int i = 0; i < 500000; ++i) {
            const std::int32_t prev = env.level;
            env.step(rate, /*decrease=*/true, /*exponential=*/true);
            if (env.level == 0) break;
            if (env.level == prev && env.counter == 0) {
                // If it made a step and didn't change level while level > 0, it stalled!
                std::fprintf(stderr, "FAIL: rate %d stalled at level %d\n", rate, env.level);
                std::exit(1);
            }
        }
        require(env.level == 0, "exponential release must reach 0");
    }
    std::fprintf(stderr, "[spu2] all 32 release rates reach level 0: PASS\n");
}

// Test that when level is 0, stepping further does not underflow below 0
void test_level_zero_does_not_underflow() {
    Envelope env;
    env.level = 0;
    for (int r = 0; r < 128; ++r) {
        env.step(r, /*decrease=*/true, /*exponential=*/true);
        require(env.level == 0, "level 0 must remain 0");
        env.step(r, /*decrease=*/true, /*exponential=*/false);
        require(env.level == 0, "level 0 must remain 0");
    }
    std::fprintf(stderr, "[spu2] level 0 does not underflow: PASS\n");
}

// Test linear decrease reaches 0
void test_linear_decrease() {
    for (int r = 0; r < 32; ++r) {
        Envelope env;
        env.level = 0x7FFF;
        for (int i = 0; i < 500000; ++i) {
            env.step(r << 2, /*decrease=*/true, /*exponential=*/false);
            if (env.level == 0) break;
        }
        require(env.level == 0, "linear decrease must reach 0");
    }
    std::fprintf(stderr, "[spu2] linear decrease reaches 0: PASS\n");
}

// Test linear and exponential attack reach max level 0x7FFF
void test_attack_reaches_max() {
    Envelope env_lin;
    env_lin.level = 0;
    for (int i = 0; i < 500000; ++i) {
        env_lin.step(64, /*decrease=*/false, /*exponential=*/false);
        if (env_lin.level >= 0x7FFF) break;
    }
    require(env_lin.level == 0x7FFF, "linear attack reaches 0x7FFF");

    Envelope env_exp;
    env_exp.level = 0;
    for (int i = 0; i < 500000; ++i) {
        env_exp.step(64, /*decrease=*/false, /*exponential=*/true);
        if (env_exp.level >= 0x7FFF) break;
    }
    require(env_exp.level == 0x7FFF, "exponential attack reaches 0x7FFF");
    std::fprintf(stderr, "[spu2] attack reaches max: PASS\n");
}

// Test that simulated Voice release phase transitions to kOff once level hits 0
void test_voice_release_transitions_to_off() {
    enum Phase { kOff, kAttack, kDecay, kSustain, kRelease };
    struct MockVoice {
        Phase phase = kRelease;
        Envelope env;
        std::uint16_t adsr2 = (7 << 6) | (1 << 5) | 14; // rate = 14 << 2, exp = 1
    } voice;

    voice.env.level = 500; // faint residual audio

    while (voice.phase != kOff) {
        // Equivalent to update_envelope in spu2.cpp
        voice.env.step((voice.adsr2 & 0x1F) << 2, true, voice.adsr2 & 0x20u);
        if (voice.env.level <= 0) voice.phase = kOff;
    }

    require(voice.phase == kOff, "voice must transition to kOff");
    require(voice.env.level == 0, "envelope level must be 0");
    std::fprintf(stderr, "[spu2] voice release transitions to kOff: PASS\n");
}

} // namespace

int main() {
    test_all_release_rates_decay_to_zero();
    test_level_zero_does_not_underflow();
    test_linear_decrease();
    test_attack_reaches_max();
    test_voice_release_transitions_to_off();
    std::fprintf(stderr, "[spu2] all envelope tests: PASS\n");
    return 0;
}
