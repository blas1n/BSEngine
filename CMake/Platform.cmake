# Create defines based on PLATFORM
if (CMAKE_SYSTEM_NAME MATCHES "Windows") 
	set (PLATFORM_WINDOWS TRUE)
	set (PLATFORM WINDOWS)
elseif (CMAKE_SYSTEM_NAME MATCHES "Linux")
	set (PLATFORM_LINUX TRUE)
	set (PLATFORM LINUX)
elseif (CMAKE_SYSTEM_NAME MATCHES "Darwin")
	set (PLATFORM_MAC TRUE)
	set (PLATFORM MAC)
endif ()

# Determine compiler
if (CMAKE_CXX_COMPILER_ID MATCHES "MSVC")
	set (DLL_IMPORT "__declspec(dllimport)" CACHE INTERNAL "DLL Import")
	set (DLL_EXPORT "__declspec(dllexport)" CACHE INTERNAL "DLL Export")

	set (COMPILER_MSVC TRUE)
	set (COMPILER MSVC)
elseif (CMAKE_CXX_COMPILER_ID MATCHES "Clang")
	set (DLL_IMPORT "__attribute__((visibility(\"default\")))" CACHE INTERNAL "DLL Import")
	set (DLL_EXPORT "__attribute__((visibility(\"default\")))" CACHE INTERNAL "DLL Export")

	set (COMPILER_CLANG TRUE)
	set (COMPILER CLANG)
elseif (CMAKE_CXX_COMPILER_ID MATCHES "GNU")
	set (DLL_IMPORT "__attribute__((visibility(\"default\")))" CACHE INTERNAL "DLL Import")
	set (DLL_EXPORT "__attribute__((visibility(\"default\")))" CACHE INTERNAL "DLL Export")

	set (COMPILER_GCC TRUE)
	set (COMPILER GCC)
endif()

# Determine target Architecture
if ((CMAKE_SYSTEM_PROCESSOR MATCHES "AMD64" OR CMAKE_SYSTEM_PROCESSOR MATCHES "x86") AND CMAKE_SIZEOF_VOID_P EQUAL 8)
	set (ARCH_x64 TRUE)
	set (ARCH x64)
elseif (CMAKE_SYSTEM_PROCESSOR MATCHES "AMD64" AND CMAKE_SIZEOF_VOID_P EQUAL 4)
	set (ARCH_x86 TRUE)
	set (ARCH x86)
endif ()

add_compile_definitions (PLATFORM=${PLATFORM} ${PLATFORM} COMPILER=${COMPILER} ${COMPILER} ARCH=${ARCH} ${ARCH})
add_compile_definitions (DLL_IMPORT=${DLL_IMPORT} DLL_EXPORT=${DLL_EXPORT})