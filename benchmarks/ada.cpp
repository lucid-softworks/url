#include <ada.h>

#include <algorithm>
#include <array>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <iomanip>
#include <iostream>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#if defined(LUCID_URL_AMALGAMATE_ADA)
#include LUCID_URL_ADA_AMALGAMATION
#endif

namespace {
constexpr std::array<std::string_view, 8> canonical = {
    "https://example.com/",
    "https://user:password@example.com:8080/path/to/resource?query=value#fragment",
    "http://www.example.org/a/b/c?one=1&two=2",
    "https://subdomain.example.co.uk/products/12345",
    "ftp://ftp.example.com/pub/file.txt",
    "ws://localhost:3000/socket",
    "https://127.0.0.1:8443/api/v1/health",
    "https://example.com/a%20path?q=already%20encoded",
};

constexpr std::array<std::string_view, 8> normalization_heavy = {
    "HTTPS://EXAMPLE.COM/a/../b?x=hello world#frag ment",
    "http://bücher.example/straße",
    "http://[2001:db8::1]:8080/a/./b",
    "file:///C|/Program Files/test.txt",
    "mailto:User@Example.com",
    "http:\\\\example.com\\a\\b",
    "http://0x7f.1/",
    "https://user name:pass word@example.com/",
};

constexpr std::array<std::string_view, 9> unicode_idna = {
    "https://bücher.example/straße",
    "https://mañana.example/café",
    "https://例え.テスト/パス",
    "https://παράδειγμα.δοκιμή/",
    "https://مثال.إختبار/",
    "https://उदाहरण.भारत/",
    "https://한국어.example/",
    "https://cafe\u0301.example/résumé",
    "https://ＥＸＡＭＰＬＥ.com/",
};

constexpr std::array<std::string_view, 3> long_scans = {
    "https://assets.example.com/packages/catalogue/components/react/dialog-manager/examples/controlled-dialog/source/index.tsx?framework=react&bundler=vite&render=client&theme=system#interactive-example",
    "https://api.example.com/v1/organizations/lucid-softworks/repositories/url/commits/306db12a15e0d5ed3428934622a187b720ae5741/check-runs?filter=latest&per_page=100",
    "https://cdn.example.com/assets/0123456789abcdefghijklmnopqrstuvwxyz/0123456789abcdefghijklmnopqrstuvwxyz/0123456789abcdefghijklmnopqrstuvwxyz/module.min.js?cache=0123456789abcdefghijklmnopqrstuvwxyz",
};

#if defined(__GNUC__) || defined(__clang__)
template <typename T>
inline void do_not_optimize(T const& value) {
  asm volatile("" : : "r,m"(value) : "memory");
}

inline std::string_view opaque_view(std::string const& input) {
  const char* data = input.data();
  std::size_t length = input.size();
  asm volatile("" : "+r"(data), "+r"(length) : : "memory");
  return std::string_view(data, length);
}
#else
template <typename T>
inline void do_not_optimize(T const& value) {
  volatile T sink = value;
  (void)sink;
}

inline std::string_view opaque_view(std::string const& input) {
  volatile const char* data = input.data();
  volatile std::size_t length = input.size();
  return std::string_view(const_cast<const char*>(data), length);
}
#endif

template <typename Range>
std::vector<std::string> materialize(Range const& inputs) {
  std::vector<std::string> out;
  out.reserve(std::size(inputs));
  for (auto const& input : inputs) {
    out.emplace_back(input);
  }
  return out;
}

std::size_t sample_ms() {
  if (const char* env = std::getenv("LUCID_URL_BENCH_SAMPLE_MS")) {
    char* end = nullptr;
    const auto value = std::strtoul(env, &end, 10);
    if (end != env && value > 0) {
      return value;
    }
  }
  return 300;
}

template <typename Url>
double sample(std::vector<std::string> const& inputs, std::size_t& checksum) {
  using clock = std::chrono::steady_clock;
  const auto minimum = std::chrono::milliseconds(sample_ms());

  {
    volatile std::size_t success = 0;
    volatile std::size_t href_size = 0;
    for (std::size_t iteration = 0; iteration < 64; ++iteration) {
      for (auto const& input : inputs) {
        auto parsed = ada::parse<Url>(opaque_view(input));
        if (!parsed) {
          std::terminate();
        }
        success = success + 1;
        auto href = parsed->get_href();
        do_not_optimize(href);
        href_size = href_size + href.size();
      }
    }
    do_not_optimize(success);
    do_not_optimize(href_size);
  }

  std::size_t iterations = 1;
  for (;;) {
    volatile std::size_t success = 0;
    volatile std::size_t href_size = 0;
    const auto start = clock::now();
    for (std::size_t iteration = 0; iteration < iterations; ++iteration) {
      for (auto const& input : inputs) {
        auto parsed = ada::parse<Url>(opaque_view(input));
        if (!parsed) {
          std::terminate();
        }
        success = success + 1;
        auto href = parsed->get_href();
        do_not_optimize(href);
        href_size = href_size + href.size();
      }
    }
    const auto elapsed = clock::now() - start;
    do_not_optimize(success);
    do_not_optimize(href_size);
    checksum ^= static_cast<std::size_t>(success) ^ static_cast<std::size_t>(href_size);
    if (elapsed >= minimum) {
      const auto nanoseconds =
          std::chrono::duration<double, std::nano>(elapsed).count();
      const auto count = static_cast<double>(iterations * inputs.size());
      return nanoseconds / count;
    }
    iterations *= 2;
  }
}

template <typename Url>
std::pair<double, double> measure(std::vector<std::string> const& inputs) {
  std::array<double, 5> samples{};
  std::size_t checksum = 0;
  for (auto& result : samples) {
    result = sample<Url>(inputs, checksum);
  }
  std::sort(samples.begin(), samples.end());
  do_not_optimize(checksum);
  const auto median = samples[samples.size() / 2];
  return {median, 1'000'000'000.0 / median};
}

// Print the corpus under test, Ada-style (# urls=…, full listing for small sets).
void print_dataset(std::string_view name,
                   std::vector<std::string> const& inputs,
                   std::string_view source) {
  std::size_t bytes = 0;
  for (auto const& input : inputs) {
    bytes += input.size();
  }
  std::size_t max_print = inputs.size() <= 64 ? inputs.size() : 8;
  if (const char* env = std::getenv("LUCID_URL_BENCH_DATASET_PRINT")) {
    char* end = nullptr;
    const auto value = std::strtoul(env, &end, 10);
    if (end != env) {
      max_print = static_cast<std::size_t>(value);
    }
  }
  std::cout << "\n# " << name << '\n';
  std::cout << "# source: " << source << '\n';
  std::cout << "# urls=" << inputs.size() << " bytes=" << bytes << '\n';
  for (std::size_t index = 0; index < inputs.size() && index < max_print;
       ++index) {
    std::cout << "#   [" << index << "] " << inputs[index] << '\n';
  }
  if (inputs.size() > max_print) {
    std::cout << "#   ... " << (inputs.size() - max_print) << " more\n";
  }
}

void print(std::string_view name, std::vector<std::string> const& inputs) {
  print_dataset(name, inputs, "inline microbenchmark corpus");
  const auto [aggregate_ns, aggregate_rate] =
      measure<ada::url_aggregator>(inputs);
  const auto [url_ns, url_rate] = measure<ada::url>(inputs);

  std::cout << "implementation             ns/url        URLs/s\n";
  std::cout << "ada   url_aggregator   " << std::fixed << std::setprecision(2)
            << std::setw(10) << aggregate_ns << "  " << std::setprecision(0)
            << std::setw(12) << aggregate_rate << '\n';
  std::cout << "ada   url              " << std::setprecision(2) << std::setw(10)
            << url_ns << "  " << std::setprecision(0) << std::setw(12)
            << url_rate << '\n';
}
}  // namespace

int main() {
  print("canonical ASCII", materialize(canonical));
  print("normalization-heavy", materialize(normalization_heavy));
  print("Unicode and IDNA", materialize(unicode_idna));
  print("long canonical scans", materialize(long_scans));
}
