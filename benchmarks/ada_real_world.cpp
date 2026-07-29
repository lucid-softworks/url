#include <ada.h>

#include <algorithm>
#include <array>
#include <chrono>
#include <cctype>
#include <cstddef>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <string>
#include <string_view>
#include <vector>

namespace {
constexpr std::array<std::string_view, 11> top_sites = {
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

template <typename Operation>
double sample(const auto& inputs, Operation operation, std::size_t& checksum) {
  using clock = std::chrono::steady_clock;
  std::size_t iterations = 1;
  for (;;) {
    const auto start = clock::now();
    for (std::size_t iteration = 0; iteration < iterations; ++iteration) {
      for (const auto& input : inputs) {
        checksum += operation(std::string_view(input));
      }
    }
    const auto elapsed = clock::now() - start;
    if (elapsed >= std::chrono::milliseconds(300)) {
      const auto nanoseconds =
          std::chrono::duration<double, std::nano>(elapsed).count();
      return nanoseconds /
             static_cast<double>(iterations * inputs.size());
    }
    iterations *= 2;
  }
}

template <typename Operation>
std::pair<double, double> measure(const auto& inputs, Operation operation) {
  std::array<double, 5> samples{};
  std::size_t checksum = 0;
  for (auto& result : samples) {
    result = sample(inputs, operation, checksum);
  }
  std::sort(samples.begin(), samples.end());
  if (checksum == 1) {
    std::cerr << checksum;
  }
  const auto median = samples[samples.size() / 2];
  return {median, 1'000'000'000.0 / median};
}

void print(const std::string_view name, const auto& inputs) {
  const auto [aggregate_ns, aggregate_rate] = measure(inputs, [](auto input) {
    auto parsed = ada::parse<ada::url_aggregator>(input);
    return parsed ? parsed->get_href_size() : 0;
  });
  const auto [url_ns, url_rate] = measure(inputs, [](auto input) {
    auto parsed = ada::parse<ada::url>(input);
    return parsed ? parsed->get_href_size() : 0;
  });
  const auto [can_parse_ns, can_parse_rate] =
      measure(inputs, [](auto input) { return std::size_t(ada::can_parse(input)); });

  std::cout << '\n' << name << " (" << inputs.size() << " URLs)\n";
  std::cout << "implementation             ns/url        URLs/s\n";
  std::cout << "ada   url_aggregator   " << std::fixed << std::setprecision(2)
            << std::setw(10) << aggregate_ns << "  " << std::setprecision(0)
            << std::setw(12) << aggregate_rate << '\n';
  std::cout << "ada   url              " << std::setprecision(2) << std::setw(10)
            << url_ns << "  " << std::setprecision(0) << std::setw(12)
            << url_rate << '\n';
  std::cout << "ada   can_parse        " << std::setprecision(2) << std::setw(10)
            << can_parse_ns << "  " << std::setprecision(0) << std::setw(12)
            << can_parse_rate << '\n';
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
  print("top sites", top_sites);
  const auto dataset = load_dataset(argv[1]);
  print("benchdata", dataset);
}
