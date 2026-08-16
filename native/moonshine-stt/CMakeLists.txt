cmake_minimum_required(VERSION 3.20)
project(beastie_moonshine_engine LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(MOONSHINE_ROOT "" CACHE PATH "Extracted Moonshine Voice native SDK")
if(NOT EXISTS "${MOONSHINE_ROOT}/include/moonshine-cpp.h")
  message(FATAL_ERROR "MOONSHINE_ROOT must contain include/moonshine-cpp.h")
endif()

add_executable(beastie-moonshine-engine main.cpp)
target_include_directories(beastie-moonshine-engine PRIVATE "${MOONSHINE_ROOT}/include")
target_link_directories(beastie-moonshine-engine PRIVATE "${MOONSHINE_ROOT}/lib")
target_link_libraries(beastie-moonshine-engine PRIVATE moonshine)

if(APPLE)
  target_link_libraries(beastie-moonshine-engine PRIVATE "-framework CoreFoundation" "-framework Foundation")
elseif(UNIX)
  set_target_properties(beastie-moonshine-engine PROPERTIES BUILD_RPATH "$ORIGIN")
endif()

if(MSVC)
  target_compile_options(beastie-moonshine-engine PRIVATE /W4 /WX)
else()
  target_compile_options(beastie-moonshine-engine PRIVATE -Wall -Wextra -Wpedantic -Werror)
endif()
