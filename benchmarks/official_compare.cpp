#include <ada.h>
#include <benchmark/benchmark.h>

#include <cctype>
#include <cstddef>
#include <fstream>
#include <iostream>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>
#include <vector>

#ifndef LUCID_URL_DATASET
#error "LUCID_URL_DATASET must identify the pinned Ada URL corpus"
#endif
#ifndef LUCID_URL_ADA_COMMIT
#error "LUCID_URL_ADA_COMMIT must identify the Ada revision"
#endif
#ifndef LUCID_URL_DATASET_COMMIT
#error "LUCID_URL_DATASET_COMMIT must identify the dataset revision"
#endif

extern "C" {
std::size_t lucid_bench_initialize(const unsigned char *path,
                                   std::size_t length);
std::size_t lucid_bench_url();
std::size_t lucid_bench_url_aggregator();
std::size_t lucid_bench_can_parse();
std::size_t lucid_bench_count_invalid();
bool lucid_bench_is_valid(std::size_t index);
}

namespace {
std::vector<std::string> url_examples;
double url_examples_bytes = 0;

std::vector<std::string> load_dataset(const std::string_view path) {
  std::ifstream file{std::string(path)};
  if (!file) {
    throw std::runtime_error("could not open URL dataset");
  }
  std::vector<std::string> urls;
  for (std::string line; std::getline(file, line);) {
    std::string_view view = line;
    while (!view.empty() &&
           std::isspace(static_cast<unsigned char>(view.back()))) {
      view.remove_suffix(1);
    }
    while (!view.empty() &&
           std::isspace(static_cast<unsigned char>(view.front()))) {
      view.remove_prefix(1);
    }
    if (!view.empty()) {
      urls.emplace_back(view);
    }
  }
  return urls;
}

void add_counters(benchmark::State &state) {
  const auto count = static_cast<double>(url_examples.size());
  state.counters["time/byte"] = benchmark::Counter(
      url_examples_bytes, benchmark::Counter::kIsIterationInvariantRate |
                              benchmark::Counter::kInvert);
  state.counters["time/url"] =
      benchmark::Counter(count, benchmark::Counter::kIsIterationInvariantRate |
                                    benchmark::Counter::kInvert);
  state.counters["speed"] = benchmark::Counter(
      url_examples_bytes, benchmark::Counter::kIsIterationInvariantRate);
  state.counters["url/s"] =
      benchmark::Counter(count, benchmark::Counter::kIsIterationInvariantRate);
}

template <class Result> void ada_parse_and_href(benchmark::State &state) {
  volatile std::size_t success = 0;
  volatile std::size_t href_size = 0;
  for (auto _ : state) {
    for (std::string &input : url_examples) {
      auto url = ada::parse<Result>(input);
      if (url) {
        success++;
        href_size += url->get_href().size();
      }
    }
  }
  (void)success;
  (void)href_size;
  add_counters(state);
}

void ada_can_parse(benchmark::State &state) {
  volatile std::size_t success = 0;
  for (auto _ : state) {
    for (std::string &input : url_examples) {
      if (ada::can_parse(input)) {
        success++;
      }
    }
  }
  (void)success;
  add_counters(state);
}

void lucid_operation(benchmark::State &state, std::size_t (*operation)()) {
  for (auto _ : state) {
    auto checksum = operation();
    benchmark::DoNotOptimize(checksum);
  }
  add_counters(state);
}

void lucid_url(benchmark::State &state) {
  lucid_operation(state, lucid_bench_url);
}

void lucid_url_aggregator(benchmark::State &state) {
  lucid_operation(state, lucid_bench_url_aggregator);
}

void lucid_can_parse(benchmark::State &state) {
  lucid_operation(state, lucid_bench_can_parse);
}

BENCHMARK_TEMPLATE(ada_parse_and_href, ada::url);
BENCHMARK_TEMPLATE(ada_parse_and_href, ada::url_aggregator);
BENCHMARK(ada_can_parse);
BENCHMARK(lucid_url);
BENCHMARK(lucid_url_aggregator);
BENCHMARK(lucid_can_parse);
} // namespace

int main(int argc, char **argv) {
  constexpr std::string_view dataset = LUCID_URL_DATASET;
  url_examples = load_dataset(dataset);
  for (const auto &input : url_examples) {
    url_examples_bytes += static_cast<double>(input.size());
  }
  const auto lucid_count = lucid_bench_initialize(
      reinterpret_cast<const unsigned char *>(dataset.data()), dataset.size());
  if (lucid_count != url_examples.size()) {
    std::cerr << "Lucid and Ada loaded different URL counts\n";
    return 2;
  }

  std::size_t ada_invalid = 0;
  std::size_t disagreements = 0;
  for (std::size_t index = 0; index < url_examples.size(); index++) {
    const auto ada_valid =
        bool(ada::parse<ada::url_aggregator>(url_examples[index]));
    const auto lucid_valid = lucid_bench_is_valid(index);
    if (!ada_valid) {
      ada_invalid++;
    }
    if (ada_valid != lucid_valid) {
      disagreements++;
      std::cerr << "validity mismatch at corpus index " << index << ": Ada "
                << ada_valid << ", Lucid " << lucid_valid << ": "
                << url_examples[index] << '\n';
    }
  }
  const auto lucid_invalid = lucid_bench_count_invalid();

  benchmark::AddCustomContext("Ada commit", LUCID_URL_ADA_COMMIT);
  benchmark::AddCustomContext("dataset commit", LUCID_URL_DATASET_COMMIT);
  benchmark::AddCustomContext("number of URLs",
                              std::to_string(url_examples.size()));
  benchmark::AddCustomContext("Ada invalid URLs", std::to_string(ada_invalid));
  benchmark::AddCustomContext("Lucid invalid URLs",
                              std::to_string(lucid_invalid));
  benchmark::AddCustomContext("validity disagreements",
                              std::to_string(disagreements));
  benchmark::Initialize(&argc, argv);
  benchmark::RunSpecifiedBenchmarks();
  benchmark::Shutdown();
}
