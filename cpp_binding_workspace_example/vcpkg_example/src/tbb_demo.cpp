#include <cmath>
#include <cstdint>
#include <functional>
#include <oneapi/tbb.h>

extern "C" int tbb_max_concurrency() {
    return oneapi::tbb::this_task_arena::max_concurrency();
}

// 用 TBB 的 parallel_reduce 并行计算 sum(sqrt(i)) for i in [0, n)
extern "C" double tbb_parallel_sum_sqrt(int64_t n) {
    return oneapi::tbb::parallel_reduce(
        oneapi::tbb::blocked_range<int64_t>(0, n), 0.0,
        [](const oneapi::tbb::blocked_range<int64_t>& r, double acc) {
            for (int64_t i = r.begin(); i != r.end(); ++i) {
                acc += std::sqrt(static_cast<double>(i));
            }
            return acc;
        },
        std::plus<double>());
}
