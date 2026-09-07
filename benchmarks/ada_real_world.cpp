#include <ada.h>

#include <algorithm>
#include <array>
#include <chrono>
#include <cctype>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <fstream>
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
// Mixed realistic inputs for parse+href. Includes IPv4, IPv6, and a non-special
// scheme so averages are not limited to the clean-HTTP fast path.
constexpr std::array<std::string_view, 11> mixed_top_sites = {
    "https://www.google.com/webhp?hl=en&amp;ictx=2&amp;sa=X&amp;ved=0ahUKEwil_oSxzJj8AhVtEFkFHTHnCGQQPQgI",
    "https://support.google.com/websearch/?p=ws_results_help&amp;hl=en-CA&amp;fg=1",
    "https://en.wikipedia.org/wiki/Dog#Roles_with_humans",
    "https://www.tiktok.com/@aguyandagolden/video/7133277734310038830",
    "https://business.twitter.com/en/help/troubleshooting/how-twitter-ads-work.html?ref=web-twc-ao-gbl-adsinfo&utm_source=twc&utm_medium=web&utm_campaign=ao&utm_content=adsinfo",
    "https://images-na.ssl-images-amazon.com/images/I/41Gc3C8UysL.css?AUIClients/AmazonGatewayAuiAssets",
    "https://www.reddit.com/?after=t3_zvz1ze",
    "https://www.reddit.com/login/?dest=https%3A%2F%2Fwww.reddit.com%2F",
    "postgresql://other:9818274x1!!@localhost:5432/otherdb?connect_timeout=10&application_name=myapp",
    "http://192.168.1.1",
    "http://[2606:4700:4700::1111]",
};

// Already-canonical special URLs that both libraries' can_parse fast paths
// target. Used so a handful of slow-path URLs cannot dominate a tiny mean.
constexpr std::array<std::string_view, 24> clean_http = {
    "https://www.google.com/",
    "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
    "https://www.facebook.com/",
    "https://www.amazon.com/dp/B08N5WRWNW",
    "https://en.wikipedia.org/wiki/URL",
    "https://www.reddit.com/r/rust/",
    "https://github.com/ada-url/ada",
    "https://stackoverflow.com/questions/tagged/url",
    "https://www.nytimes.com/",
    "https://www.bbc.com/news",
    "https://www.apple.com/iphone/",
    "https://developer.mozilla.org/en-US/docs/Web/API/URL",
    "https://crates.io/crates/url",
    "https://docs.rs/url/latest/url/",
    "http://example.com/path?query=1#frag",
    "https://cdn.example.com/static/app.js",
    "https://api.example.com/v1/users/42",
    "https://subdomain.example.co.uk/products/12345",
    "https://news.ycombinator.com/item?id=1",
    "https://www.linkedin.com/in/example/",
    "https://twitter.com/yagiznizipli",
    "https://www.instagram.com/",
    "https://www.netflix.com/browse",
    "https://www.microsoft.com/en-us/",
};

// Prevent the amalgamated Ada TU from constant-folding known string literals
// into can_parse/parse (inputs must look runtime-opaque).
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

template <typename Operation>
double sample(std::vector<std::string> const& inputs, Operation operation,
              std::size_t& checksum) {
  using clock = std::chrono::steady_clock;
  const auto minimum = std::chrono::milliseconds(sample_ms());

  // Warm one doubling ladder so the timed samples are not cold-I-cache heavy.
  {
    volatile std::size_t sink = 0;
    for (std::size_t iteration = 0; iteration < 64; ++iteration) {
      for (auto const& input : inputs) {
        sink = sink + operation(opaque_view(input));
      }
    }
    do_not_optimize(sink);
  }

  std::size_t iterations = 1;
  for (;;) {
    volatile std::size_t sink = 0;
    const auto start = clock::now();
    for (std::size_t iteration = 0; iteration < iterations; ++iteration) {
      for (auto const& input : inputs) {
        sink = sink + operation(opaque_view(input));
      }
    }
    const auto elapsed = clock::now() - start;
    do_not_optimize(sink);
    checksum ^= static_cast<std::size_t>(sink);
    if (elapsed >= minimum) {
      const auto nanoseconds =
          std::chrono::duration<double, std::nano>(elapsed).count();
      return nanoseconds / static_cast<double>(iterations * inputs.size());
    }
    iterations *= 2;
  }
}

template <typename Operation>
std::pair<double, double> measure(std::vector<std::string> const& inputs,
                                  Operation operation) {
  std::array<double, 5> samples{};
  std::size_t checksum = 0;
  for (auto& result : samples) {
    result = sample(inputs, operation, checksum);
  }
  std::sort(samples.begin(), samples.end());
  do_not_optimize(checksum);
  const auto median = samples[samples.size() / 2];
  return {median, 1'000'000'000.0 / median};
}

void print_row(std::string_view label, double nanoseconds, double rate) {
  std::cout << "ada   " << std::left << std::setw(18) << label << std::right
            << std::fixed << std::setprecision(2) << std::setw(10) << nanoseconds
            << "  " << std::setprecision(0) << std::setw(12) << rate << '\n';
}

// Print the corpus under test, Ada-style (# Loading …, # urls=…, samples).
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
  if (source.rfind("file:", 0) == 0) {
    if (const char* commit = std::getenv("ADA_DATASET_COMMIT")) {
      std::cout << "# dataset commit: " << commit << '\n';
    }
  }
  std::cout << "# urls=" << inputs.size() << " bytes=" << bytes << '\n';
  for (std::size_t index = 0; index < inputs.size() && index < max_print;
       ++index) {
    std::cout << "#   [" << index << "] " << inputs[index] << '\n';
  }
  if (inputs.size() > max_print) {
    std::cout << "#   ... " << (inputs.size() - max_print) << " more\n";
  }
}

void print_parse(std::string_view name, std::vector<std::string> const& inputs,
                 std::string_view source) {
  print_dataset(name, inputs, source);
  const auto [aggregate_ns, aggregate_rate] = measure(inputs, [](auto input) {
    auto parsed = ada::parse<ada::url_aggregator>(input);
    if (!parsed) {
      return std::size_t{0};
    }
    auto href = parsed->get_href();
    do_not_optimize(href);
    return href.size();
  });
  const auto [url_ns, url_rate] = measure(inputs, [](auto input) {
    auto parsed = ada::parse<ada::url>(input);
    if (!parsed) {
      return std::size_t{0};
    }
    auto href = parsed->get_href();
    do_not_optimize(href);
    return href.size();
  });

  std::cout << "implementation             ns/url        URLs/s\n";
  print_row("url_aggregator", aggregate_ns, aggregate_rate);
  print_row("url", url_ns, url_rate);
}

void print_can_parse(std::string_view name,
                     std::vector<std::string> const& inputs,
                     std::string_view source) {
  print_dataset(name, inputs, source);
  const auto [can_parse_ns, can_parse_rate] = measure(
      inputs, [](auto input) { return std::size_t(ada::can_parse(input)); });

  std::cout << "implementation             ns/url        URLs/s\n";
  print_row("can_parse", can_parse_ns, can_parse_rate);
}

void print_all(std::string_view name, std::vector<std::string> const& inputs,
               std::string_view source) {
  print_parse(name, inputs, source);
  const auto [can_parse_ns, can_parse_rate] = measure(
      inputs, [](auto input) { return std::size_t(ada::can_parse(input)); });
  print_row("can_parse", can_parse_ns, can_parse_rate);
}

std::vector<std::string> load_dataset(const char* path) {
  std::ifstream file(path);
  if (!file) {
    throw std::runtime_error("could not open dataset");
  }
  std::vector<std::string> urls;
  for (std::string line; std::getline(file, line);) {
    auto first = line.begin();
    while (first != line.end() &&
           std::isspace(static_cast<unsigned char>(*first))) {
      ++first;
    }
    auto last = line.end();
    while (last != first &&
           std::isspace(static_cast<unsigned char>(*(last - 1)))) {
      --last;
    }
    if (first != last) {
      urls.emplace_back(first, last);
    }
  }
  return urls;
}
}  // namespace

int main(int argc, char** argv) {
  if (argc != 2) {
    std::cerr << "usage: ada-real-world <dataset>\n";
    return 2;
  }

  std::cout
      << "note: mixed top sites measure parse+href only (includes IPv4/IPv6/"
         "non-special).\n"
      << "note: clean HTTP measures can_parse on already-canonical special "
         "URLs.\n"
      << "note: benchdata reports parse+href and can_parse on the full "
         "corpus.\n"
      << "note: each section prints the dataset (urls/bytes/source); set "
         "LUCID_URL_BENCH_DATASET_PRINT=N to cap listed URLs.\n";

  // Same default set as ada-url/ada benchmarks/bench.cpp url_examples_default.
  print_parse("mixed top sites parse", materialize(mixed_top_sites),
              "inline (ada-url/ada bench.cpp url_examples_default)");
  print_can_parse("clean HTTP can_parse", materialize(clean_http),
                  "inline clean-HTTP can_parse corpus");

  std::cout << "\n# Loading " << argv[1] << '\n';
  const auto dataset = load_dataset(argv[1]);
  const std::string source =
      std::string("file:") + argv[1] + " (ada-url/url-dataset out.txt)";
  print_all("benchdata", dataset, source);
}
