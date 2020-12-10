# Source/Runtime/Launch

add_executable(${CMAKE_PROJECT_NAME} "Launch.cpp")

target_link_libraries (${CMAKE_PROJECT_NAME} PRIVATE
	Engine
	Renderer
	Thread
)