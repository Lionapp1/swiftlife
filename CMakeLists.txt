cmake_minimum_required(VERSION 3.20)
project(swiftlife VERSION 0.2.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

find_package(PkgConfig REQUIRED)
pkg_check_modules(GTK3 REQUIRED IMPORTED_TARGET gtk+-3.0)
pkg_check_modules(WEBKIT REQUIRED IMPORTED_TARGET webkit2gtk-4.1)

add_executable(swiftlife src/main.cpp)
target_link_libraries(swiftlife PRIVATE PkgConfig::GTK3 PkgConfig::WEBKIT)
target_compile_options(swiftlife PRIVATE -Wall -Wextra -Wpedantic)

include(GNUInstallDirs)
install(TARGETS swiftlife RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
install(FILES packaging/swiftlife.desktop DESTINATION ${CMAKE_INSTALL_DATADIR}/applications)
install(FILES packaging/swiftlife.svg DESTINATION ${CMAKE_INSTALL_DATADIR}/icons/hicolor/scalable/apps)
