# Create defines based on platform
if (CMAKE_SYSTEM_NAME MATCHES "Windows") 
	set (platform_windows TRUE) 
elseif (CMAKE_SYSTEM_NAME MATCHES "Linux")
	set (platform_linux TRUE)
elseif (CMAKE_SYSTEM_NAME MATCHES "Darwin")
	set (platform_mac TRUE)
endif ()

# Determine target architecture
if ((CMAKE_SYSTEM_PROCESSOR MATCHES "AMD64" OR CMAKE_SYSTEM_PROCESSOR MATCHES "x86") AND CMAKE_SIZEOF_VOID_P EQUAL 8)
	set (arch_X64 TRUE)
elseif (CMAKE_SYSTEM_PROCESSOR MATCHES "AMD64" AND CMAKE_SIZEOF_VOID_P EQUAL 4)
	set (arch_X86 TRUE)
endif ()

# Determine compiler
if (cmake_cxx_compiler_id MATCHES "MSVC")
	set (compiler_msvc TRUE)
elseif (cmake_cxx_compiler_id MATCHES "Clang")
	set (compiler_clang TRUE)
elseif (cmake_cxx_compiler_id MATCHES "GNU")
	set (compiler_gcc TRUE)
endif()