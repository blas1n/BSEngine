cmake_minimum_required (VERSION 3.8)

set(ENGINE_SRC
	include/Allocator.h
	include/Array.h
	include/Core.h
	include/Deque.h
	include/HeapMemory.h
	include/IManager.h
	include/Interface.h
	include/List.h
	include/Macro.h
	include/Map.h
	include/MathFunctions.h
	include/Matrix3x3.h
	include/Matrix4x4.h
	include/MemoryManager.h
	include/Quaternion.h
	include/Queue.h
	include/Random.h
	include/Set.h
	include/Stack.h
	include/System.h
	include/ThreadManager.h
	include/Type.h
	include/Vector2.h
	include/Vector3.h
	include/Vector4.h

	src/HeapMemory.cpp
	src/main.cpp
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

include_directories (include)

add_executable (engine ${ENGINE_SRC})

set_target_properties (engine PROPERTIES
	OUTPUT_NAME engine
	CXX_STANDARD 17
    CXX_EXTENSIONS OFF
)

foreach(FILE ${ENGINE_SRC}) 
    get_filename_component(FILE_EXT "${FILE}" LAST_EXT)

    if ("${FILE_EXT}" MATCHES ".cpp")
       set(GROUP src)
    elseif("${FILE_EXT}" MATCHES ".h")
       set(GROUP include)
    endif()

    source_group("${GROUP}" FILES "${FILE}")
endforeach()