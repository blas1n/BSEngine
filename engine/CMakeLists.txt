cmake_minimum_required (VERSION 3.8)

set(ENGINE_SRC
	include/AccesserGenerator.h
	include/Character.h
	include/ComponentAllocator.h
	include/ComponentMemory.h
	include/Core.h
	include/CString.h
	include/Exception.h
	include/HeapMemory.h
	include/HeapAllocator.h
	include/Interface.h
	include/Iterator.h
	include/Macro.h
	include/ManagerAllocator.h
	include/ManagerMemory.h
	include/MathFunctions.h
	include/Matrix3x3.h
	include/Matrix4x4.h
	include/MemoryManager.h
	include/OneFrameAllocator.h
	include/OneFrameMemory.h
	include/Quaternion.h
	include/Random.h
	include/ResourceAllocator.h
	include/ResourceMemory.h
	include/System.h
	include/ThreadManager.h
	include/Type.h
	include/Vector2.h
	include/Vector3.h
	include/Vector4.h

	src/HeapMemory.cpp
	src/Main_Linux.cpp
	src/Main_Windows.cpp
	src/MathFunctions.cpp
	src/Matrix3x3.cpp
	src/Matrix4x4.cpp
	src/MemoryManager.cpp
	src/Quaternion.cpp
	src/Random.cpp
	src/System.cpp
	src/ThreadManager.cpp
	src/Vector2.cpp
	src/Vector3.cpp
	src/Vector4.cpp
)

if (CMAKE_SYSTEM_NAME MATCHES "Linux")
find_package (Threads)
endif ()

include_directories (include)

if (WIN32)
add_executable (engine WIN32 ${ENGINE_SRC})
else ()
add_executable (engine ${ENGINE_SRC})
endif()

target_link_libraries (engine ${CMAKE_THREAD_LIBS_INIT})

set_target_properties (engine PROPERTIES
	OUTPUT_NAME engine
	CXX_STANDARD 17
    CXX_EXTENSIONS OFF
)

if (WIN32)
foreach(FILE ${ENGINE_SRC}) 
    get_filename_component(FILE_EXT "${FILE}" EXT)

    if ("${FILE_EXT}" MATCHES ".cpp")
       set(GROUP src)
    elseif("${FILE_EXT}" MATCHES ".h")
       set(GROUP include)
    endif()

    source_group("${GROUP}" FILES "${FILE}")
endforeach()
endif()