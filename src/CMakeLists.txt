cmake_minimum_required(VERSION 3.16)
project(BingWallpaperSetter VERSION 1.0.1 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# 设置自动处理Qt的MOC、UIC和RCC
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

# 查找Qt5或Qt6
find_package(QT NAMES Qt6 Qt5 REQUIRED COMPONENTS Core)
find_package(Qt${QT_VERSION_MAJOR} REQUIRED COMPONENTS
    Core
    Gui
    Widgets
    Network
)

# 源文件（从当前目录读取）
set(PROJECT_SOURCES
    ${CMAKE_CURRENT_SOURCE_DIR}/main.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/MainWindow.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/MainWindow.h
    ${CMAKE_CURRENT_SOURCE_DIR}/BingWallpaperSetter.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/BingWallpaperSetter.h
)

# 创建可执行文件
add_executable(${PROJECT_NAME} ${PROJECT_SOURCES})

# 链接Qt库
target_link_libraries(${PROJECT_NAME} PRIVATE
    Qt${QT_VERSION_MAJOR}::Core
    Qt${QT_VERSION_MAJOR}::Gui
    Qt${QT_VERSION_MAJOR}::Widgets
    Qt${QT_VERSION_MAJOR}::Network
)

# 设置输出目录到项目根目录的build/bin
set_target_properties(${PROJECT_NAME} PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin"
)

# 安装规则
install(TARGETS ${PROJECT_NAME}
    RUNTIME DESTINATION bin
)

# 安装desktop文件
install(FILES ${CMAKE_CURRENT_SOURCE_DIR}/bing-wallpaper-setter.desktop
    DESTINATION share/applications
)

# 打印编译信息
message(STATUS "Qt版本: ${QT_VERSION}")
message(STATUS "编译器: ${CMAKE_CXX_COMPILER_ID} ${CMAKE_CXX_COMPILER_VERSION}")
message(STATUS "C++标准: ${CMAKE_CXX_STANDARD}")
