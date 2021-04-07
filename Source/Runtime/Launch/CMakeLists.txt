# Source/Runtime/Launch

if (MSVC)
	add_executable (${CMAKE_PROJECT_NAME} WIN32 "Launch.cpp")
else ()
	add_executable (${CMAKE_PROJECT_NAME} "Launch.cpp")
endif ()

target_link_libraries (${CMAKE_PROJECT_NAME} PRIVATE
	Engine
	Renderer
	Thread
)