#include <ada.h>

#include <algorithm>
#include <array>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <iomanip>
#include <iostream>
#include <string_view>

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

template <typename Url>
double sample(const auto& inputs, std::size_t& checksum) {
  using clock = std::chrono::steady_clock;
  std::size_t iterations = 1;
  for (;;) {
    volatile std::size_t success = 0;
    volatile std::size_t href_size = 0;
    const auto start = clock::now();
    for (std::size_t iteration = 0; iteration < iterations; ++iteration) {
      for (const auto input : inputs) {
        auto parsed = ada::parse<Url>(input);
        if (!parsed) {
          std::terminate();
        }
        success = success + 1;
        href_size = href_size + parsed->get_href().size();
      }
    }
    const auto elapsed = clock::now() - start;
    checksum ^= static_cast<std::size_t>(success) ^ static_cast<std::size_t>(href_size);
    if (elapsed >= std::chrono::milliseconds(300)) {
      const auto nanoseconds =
          std::chrono::duration<double, std::nano>(elapsed).count();
      const auto count = static_cast<double>(iterations * inputs.size());
      return nanoseconds / count;
    }
    iterations *= 2;
  }
}

template <typename Url>
std::pair<double, double> measure(const auto& inputs) {
  std::array<double, 5> samples{};
  std::size_t checksum = 0;
  for (auto& result : samples) {
    result = sample<Url>(inputs, checksum);
  }
  std::sort(samples.begin(), samples.end());
  if (checksum == 1) {
    std::cerr << checksum;
  }
  const auto median = samples[samples.size() / 2];
  return {median, 1'000'000'000.0 / median};
}

void print(const std::string_view name, const auto& inputs) {
  const auto [aggregate_ns, aggregate_rate] =
      measure<ada::url_aggregator>(inputs);
  const auto [url_ns, url_rate] = measure<ada::url>(inputs);

  std::cout << '\n' << name << '\n';
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
  print("canonical ASCII", canonical);
  print("normalization-heavy", normalization_heavy);
  print("Unicode and IDNA", unicode_idna);
  print("long canonical scans", long_scans);
}
