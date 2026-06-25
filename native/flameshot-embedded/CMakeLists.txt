cmake_minimum_required(VERSION 3.22)

project(codex_flameshot_embedded
        VERSION 14.0.0
        LANGUAGES CXX)

if(NOT DEFINED FLAMESHOT_SOURCE_DIR)
    message(FATAL_ERROR "Set FLAMESHOT_SOURCE_DIR to a Flameshot source checkout.")
endif()
get_filename_component(FLAMESHOT_SOURCE_DIR "${FLAMESHOT_SOURCE_DIR}" ABSOLUTE)
if(NOT EXISTS "${FLAMESHOT_SOURCE_DIR}/src/core/flameshot.h")
    message(FATAL_ERROR "FLAMESHOT_SOURCE_DIR does not look like Flameshot source: ${FLAMESHOT_SOURCE_DIR}")
endif()
get_filename_component(CODEX_FLAMESHOT_VENDOR_DIR "${FLAMESHOT_SOURCE_DIR}/.." ABSOLUTE)

set(QT_DEFAULT_MAJOR_VERSION 6 CACHE STRING "")
set(QT_VERSION_MAJOR 6 CACHE STRING "")
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

set(ENABLE_IMGUR OFF CACHE BOOL "" FORCE)
set(DISABLE_UPDATE_CHECKER ON CACHE BOOL "" FORCE)
set(USE_WAYLAND_CLIPBOARD OFF CACHE BOOL "" FORCE)
set(USE_LAUNCHER_ABSOLUTE_PATH OFF CACHE BOOL "" FORCE)
if(WIN32 OR APPLE)
    set(USE_KDSINGLEAPPLICATION ON CACHE BOOL "" FORCE)
    set(USE_BUNDLED_KDSINGLEAPPLICATION ON CACHE BOOL "" FORCE)
else()
    set(USE_KDSINGLEAPPLICATION OFF CACHE BOOL "" FORCE)
endif()

include(FetchContent)

find_package(Qt6 CONFIG REQUIRED Core Gui Widgets Network Svg)
if(UNIX AND NOT APPLE)
    find_package(Qt6 CONFIG REQUIRED DBus)
endif()

set(CODEX_QT_COLOR_WIDGETS_SOURCE_DIR "" CACHE PATH "Qt-Color-Widgets source checkout")
if(NOT CODEX_QT_COLOR_WIDGETS_SOURCE_DIR
   AND EXISTS "${CODEX_FLAMESHOT_VENDOR_DIR}/qt-color-widgets-src/CMakeLists.txt")
    set(CODEX_QT_COLOR_WIDGETS_SOURCE_DIR "${CODEX_FLAMESHOT_VENDOR_DIR}/qt-color-widgets-src")
endif()

if(CODEX_QT_COLOR_WIDGETS_SOURCE_DIR)
    add_subdirectory("${CODEX_QT_COLOR_WIDGETS_SOURCE_DIR}"
                     "${CMAKE_CURRENT_BINARY_DIR}/qt-color-widgets"
                     EXCLUDE_FROM_ALL)
else()
    FetchContent_Declare(
        qtColorWidgets
        GIT_REPOSITORY https://gitlab.com/mattbas/Qt-Color-Widgets.git
        GIT_TAG 5d52e907e50dc88cf969b41cea44665ff6c475b1)
    FetchContent_MakeAvailable(qtColorWidgets)
endif()

if(WIN32 OR APPLE)
    set(KDSingleApplication_QT6 ON CACHE BOOL "" FORCE)
    set(KDSingleApplication_STATIC ON CACHE BOOL "" FORCE)
    set(KDSingleApplication_TESTS OFF CACHE BOOL "" FORCE)
    set(KDSingleApplication_EXAMPLES OFF CACHE BOOL "" FORCE)
    set(KDSingleApplication_DOCS OFF CACHE BOOL "" FORCE)
    set(CODEX_KDSINGLEAPPLICATION_SOURCE_DIR "" CACHE PATH "KDSingleApplication source checkout")
    if(NOT CODEX_KDSINGLEAPPLICATION_SOURCE_DIR
       AND EXISTS "${CODEX_FLAMESHOT_VENDOR_DIR}/kdsingleapplication-src/CMakeLists.txt")
        set(CODEX_KDSINGLEAPPLICATION_SOURCE_DIR "${CODEX_FLAMESHOT_VENDOR_DIR}/kdsingleapplication-src")
    endif()
    if(CODEX_KDSINGLEAPPLICATION_SOURCE_DIR)
        add_subdirectory("${CODEX_KDSINGLEAPPLICATION_SOURCE_DIR}"
                         "${CMAKE_CURRENT_BINARY_DIR}/kdsingleapplication"
                         EXCLUDE_FROM_ALL)
    else()
        FetchContent_Declare(
            kdsingleApplication
            GIT_REPOSITORY https://github.com/KDAB/KDSingleApplication.git
            GIT_TAG v1.2.0)
        FetchContent_MakeAvailable(kdsingleApplication)
    endif()

    set(QHOTKEY_INSTALL OFF CACHE BOOL "" FORCE)
    set(QHOTKEY_EXAMPLES OFF CACHE BOOL "" FORCE)
    set(CODEX_QHOTKEY_SOURCE_DIR "" CACHE PATH "QHotkey source checkout")
    if(NOT CODEX_QHOTKEY_SOURCE_DIR
       AND EXISTS "${CODEX_FLAMESHOT_VENDOR_DIR}/qhotkey-src/CMakeLists.txt")
        set(CODEX_QHOTKEY_SOURCE_DIR "${CODEX_FLAMESHOT_VENDOR_DIR}/qhotkey-src")
    endif()
    if(CODEX_QHOTKEY_SOURCE_DIR)
        add_subdirectory("${CODEX_QHOTKEY_SOURCE_DIR}"
                         "${CMAKE_CURRENT_BINARY_DIR}/qhotkey"
                         EXCLUDE_FROM_ALL)
    else()
        FetchContent_Declare(
            qHotKey
            GIT_REPOSITORY https://github.com/flameshot-org/QHotkey
            GIT_TAG d7063877c14d5ae2b489dc70bbe02e76a43bf38b)
        FetchContent_MakeAvailable(qHotKey)
    endif()
endif()

add_library(flameshot SHARED
    codex_flameshot_bridge.cpp
    "${FLAMESHOT_SOURCE_DIR}/data/graphics.qrc")
set_target_properties(flameshot PROPERTIES
    OUTPUT_NAME codex_flameshot_embedded
    MACOSX_RPATH ON
    INSTALL_NAME_DIR "@rpath")
target_compile_features(flameshot PUBLIC cxx_std_20)
target_include_directories(flameshot
    PUBLIC
        "${CMAKE_CURRENT_SOURCE_DIR}"
        "${FLAMESHOT_SOURCE_DIR}/src"
        "${CMAKE_CURRENT_BINARY_DIR}")
target_compile_definitions(flameshot
    PRIVATE
        CODEX_FLAMESHOT_BUILDING_DLL
        CODEX_FLAMESHOT_EMBEDDED
        DISABLE_UPDATE_CHECKER
        APP_PREFIX=""
        APP_VERSION="v${PROJECT_VERSION}"
        FLAMESHOT_APP_VERSION_URL=""
        FLAMESHOT_GIT_HASH="codex-embedded"
        PREDEFINED_COLOR_PALETTE_LARGE=false)
if(MSVC)
    target_compile_options(flameshot PRIVATE /FIQtCore/QCoreApplication)
endif()

if(WIN32)
    target_compile_definitions(flameshot
        PRIVATE
            USE_PORTABLE_CONFIG
            FLAMESHOT_VERSION_MAJOR=${PROJECT_VERSION_MAJOR}
            FLAMESHOT_VERSION_MINOR=${PROJECT_VERSION_MINOR}
            FLAMESHOT_VERSION_BUGFIX=${PROJECT_VERSION_PATCH}
            FLAMESHOT_VERSION_BUILD=1
            FLAMESHOT_VERSION_STRING="${PROJECT_VERSION}")
endif()

add_subdirectory("${FLAMESHOT_SOURCE_DIR}/src/config"
                 "${CMAKE_CURRENT_BINARY_DIR}/flameshot-config")
add_subdirectory("${FLAMESHOT_SOURCE_DIR}/src/core"
                 "${CMAKE_CURRENT_BINARY_DIR}/flameshot-core")
add_subdirectory("${FLAMESHOT_SOURCE_DIR}/src/utils"
                 "${CMAKE_CURRENT_BINARY_DIR}/flameshot-utils")
add_subdirectory("${FLAMESHOT_SOURCE_DIR}/src/widgets"
                 "${CMAKE_CURRENT_BINARY_DIR}/flameshot-widgets")
add_subdirectory("${FLAMESHOT_SOURCE_DIR}/src/tools"
                 "${CMAKE_CURRENT_BINARY_DIR}/flameshot-tools")

target_link_libraries(flameshot
    PRIVATE
        Qt6::Svg
        Qt6::Network
        Qt6::Widgets
        QtColorWidgets)
if(UNIX AND NOT APPLE)
    target_link_libraries(flameshot PRIVATE Qt6::DBus)
endif()
if(WIN32 OR APPLE)
    target_link_libraries(flameshot PRIVATE qhotkey)
    target_link_libraries(flameshot PRIVATE KDAB::kdsingleapplication)
    target_compile_definitions(flameshot PRIVATE USE_KDSINGLEAPPLICATION=1)
endif()
