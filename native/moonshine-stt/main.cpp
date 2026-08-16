#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <limits>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

#include "moonshine-cpp.h"

namespace {
constexpr int32_t kSampleRate = 16000;
constexpr size_t kChunkSamples = 342;
constexpr size_t kUpdateSamples = 7696;
constexpr size_t kEndpointPaddingSamples = kSampleRate;

struct Request {
  uint64_t id;
  std::string audio_key;
};

uint16_t read_u16(std::istream& input) {
  uint8_t bytes[2]{};
  input.read(reinterpret_cast<char*>(bytes), sizeof(bytes));
  if (!input) throw std::runtime_error("truncated WAV");
  return static_cast<uint16_t>(bytes[0]) |
         (static_cast<uint16_t>(bytes[1]) << 8);
}

uint32_t read_u32(std::istream& input) {
  uint8_t bytes[4]{};
  input.read(reinterpret_cast<char*>(bytes), sizeof(bytes));
  if (!input) throw std::runtime_error("truncated WAV");
  return static_cast<uint32_t>(bytes[0]) |
         (static_cast<uint32_t>(bytes[1]) << 8) |
         (static_cast<uint32_t>(bytes[2]) << 16) |
         (static_cast<uint32_t>(bytes[3]) << 24);
}

std::vector<float> load_wav(const std::filesystem::path& path) {
  std::ifstream input(path, std::ios::binary);
  if (!input) throw std::runtime_error("audio unavailable");
  char marker[4]{};
  input.read(marker, 4);
  if (std::string(marker, 4) != "RIFF") throw std::runtime_error("invalid WAV");
  (void)read_u32(input);
  input.read(marker, 4);
  if (std::string(marker, 4) != "WAVE") throw std::runtime_error("invalid WAV");

  bool format_found = false;
  uint16_t format = 0;
  uint16_t channels = 0;
  uint32_t sample_rate = 0;
  uint16_t bits = 0;
  std::vector<int16_t> pcm;
  while (input.read(marker, 4)) {
    const uint32_t size = read_u32(input);
    const std::string chunk(marker, 4);
    if (chunk == "fmt ") {
      if (size < 16) throw std::runtime_error("invalid WAV format");
      format = read_u16(input);
      channels = read_u16(input);
      sample_rate = read_u32(input);
      (void)read_u32(input);
      (void)read_u16(input);
      bits = read_u16(input);
      input.seekg(size - 16, std::ios::cur);
      format_found = true;
    } else if (chunk == "data") {
      if (!format_found || size % 2 != 0) throw std::runtime_error("invalid WAV data");
      pcm.resize(size / 2);
      input.read(reinterpret_cast<char*>(pcm.data()), size);
      if (!input) throw std::runtime_error("truncated WAV data");
    } else {
      input.seekg(size, std::ios::cur);
    }
    if (size % 2 != 0) input.seekg(1, std::ios::cur);
  }
  if (format != 1 || channels != 1 || sample_rate != kSampleRate || bits != 16 ||
      pcm.empty()) {
    throw std::runtime_error("unsupported WAV format");
  }
  std::vector<float> samples;
  samples.reserve(pcm.size() + kEndpointPaddingSamples);
  for (int16_t sample : pcm) {
    samples.push_back(static_cast<float>(sample) / 32768.0f);
  }
  samples.insert(samples.end(), kEndpointPaddingSamples, 0.0f);
  return samples;
}

uint64_t parse_unsigned(const std::string& line, const std::string& field) {
  const std::string prefix = "\"" + field + "\":";
  const size_t start = line.find(prefix);
  if (start == std::string::npos) throw std::runtime_error("missing request field");
  const size_t value_start = start + prefix.size();
  size_t value_end = value_start;
  while (value_end < line.size() && line[value_end] >= '0' && line[value_end] <= '9') {
    ++value_end;
  }
  if (value_end == value_start) throw std::runtime_error("invalid request ID");
  return std::stoull(line.substr(value_start, value_end - value_start));
}

std::string parse_string(const std::string& line, const std::string& field) {
  const std::string prefix = "\"" + field + "\":\"";
  const size_t start = line.find(prefix);
  if (start == std::string::npos) throw std::runtime_error("missing request field");
  const size_t value_start = start + prefix.size();
  const size_t value_end = line.find('"', value_start);
  if (value_end == std::string::npos) throw std::runtime_error("invalid request field");
  return line.substr(value_start, value_end - value_start);
}

Request parse_request(const std::string& line) {
  if (parse_unsigned(line, "protocol_version") != 1 ||
      parse_string(line, "language") != "english") {
    throw std::runtime_error("unsupported recognition request");
  }
  Request request{parse_unsigned(line, "request_id"), parse_string(line, "audio_key")};
  if (request.id == 0 || request.audio_key.size() != 64 ||
      !std::all_of(request.audio_key.begin(), request.audio_key.end(), [](char character) {
        return (character >= '0' && character <= '9') ||
               (character >= 'a' && character <= 'f');
      })) {
    throw std::runtime_error("invalid recognition request");
  }
  return request;
}

std::string json_escape(const std::string& text) {
  std::ostringstream escaped;
  for (unsigned char character : text) {
    switch (character) {
      case '"': escaped << "\\\""; break;
      case '\\': escaped << "\\\\"; break;
      case '\b': escaped << "\\b"; break;
      case '\f': escaped << "\\f"; break;
      case '\n': escaped << "\\n"; break;
      case '\r': escaped << "\\r"; break;
      case '\t': escaped << "\\t"; break;
      default:
        if (character < 0x20) {
          const char* hex = "0123456789abcdef";
          escaped << "\\u00" << hex[character >> 4] << hex[character & 0xf];
        } else {
          escaped << character;
        }
    }
  }
  return escaped.str();
}

struct Recognition {
  std::string text;
  uint16_t confidence;
};

Recognition recognize(moonshine::Transcriber& transcriber,
                      const std::vector<float>& audio) {
  moonshine::Stream stream = transcriber.createStream(0.48);
  stream.start();
  size_t since_update = 0;
  for (size_t offset = 0; offset < audio.size(); offset += kChunkSamples) {
    const size_t end = std::min(offset + kChunkSamples, audio.size());
    stream.addAudio(std::vector<float>(audio.begin() + offset, audio.begin() + end),
                    kSampleRate);
    since_update += end - offset;
    if (since_update >= kUpdateSamples) {
      (void)stream.updateTranscription();
      since_update = 0;
    }
  }
  const moonshine::Transcript transcript = stream.updateTranscription();
  stream.stop();

  std::ostringstream text;
  bool all_complete = true;
  for (const auto& line : transcript.lines) {
    if (!line.text.empty()) {
      if (text.tellp() > 0) text << ' ';
      text << line.text;
    }
    all_complete = all_complete && line.isComplete;
  }
  std::string final_text = text.str();
  while (!final_text.empty() && std::isspace(static_cast<unsigned char>(final_text.back()))) {
    final_text.pop_back();
  }
  // Tiny Streaming's optional word-alignment decoder adds tens of seconds to a short utterance
  // on the development Mac, so it is deliberately not part of the release bundle. This is a
  // conservative usability score based on VAD plus endpoint completion, not a claimed probability
  // that every word is correct. The game displays uncertain recognition and never turns it into
  // authoritative state directly.
  const uint16_t confidence = final_text.empty() ? 0 : (all_complete ? 800 : 600);
  return Recognition{final_text, confidence};
}

void write_error(uint64_t request_id, const std::string& code) {
  std::cout << "{\"protocol_version\":1,\"request_id\":" << request_id
            << ",\"outcome\":{\"status\":\"error\",\"code\":\"" << code
            << "\"}}\n" << std::flush;
}
}  // namespace

int main(int argc, char** argv) {
  std::filesystem::path model_dir;
  std::filesystem::path audio_root;
  for (int index = 1; index < argc; ++index) {
    const std::string argument = argv[index];
    if (argument == "--model-dir" && index + 1 < argc) {
      model_dir = argv[++index];
    } else if (argument == "--audio-root" && index + 1 < argc) {
      audio_root = argv[++index];
    } else {
      std::cerr << "usage: beastie-moonshine-engine --model-dir PATH --audio-root PATH\n";
      return 2;
    }
  }
  if (model_dir.empty() || audio_root.empty()) return 2;

  try {
    moonshine::Transcriber transcriber(
        model_dir.string(), moonshine::ModelArch::TINY_STREAMING, 0.48);
    std::string line;
    while (std::getline(std::cin, line)) {
      uint64_t request_id = 0;
      try {
        const Request request = parse_request(line);
        request_id = request.id;
        const Recognition result = recognize(
            transcriber, load_wav(audio_root / (request.audio_key + ".wav")));
        if (result.text.empty()) {
          std::cout << "{\"protocol_version\":1,\"request_id\":" << request.id
                    << ",\"outcome\":{\"status\":\"no_speech\"}}\n" << std::flush;
        } else {
          std::cout << "{\"protocol_version\":1,\"request_id\":" << request.id
                    << ",\"outcome\":{\"status\":\"recognized\",\"text\":\""
                    << json_escape(result.text) << "\",\"confidence\":"
                    << result.confidence << "}}\n" << std::flush;
        }
      } catch (const std::exception&) {
        write_error(request_id, "recognition_failed");
      }
    }
  } catch (const std::exception& error) {
    std::cerr << "Moonshine initialization failed: " << error.what() << '\n';
    return 1;
  }
  return 0;
}
