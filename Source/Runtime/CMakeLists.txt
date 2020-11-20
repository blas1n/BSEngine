# Source/Runtime

if (${PLATFORM_WINDOWS})
	set (DLL_IMPORT __declspec(dllimport))
	set (DLL_EXPORT __declspec(dllexport))
elseif (${PLATFORM_LINUX})
	set (DLL_IMPORT __attribute__((visibility("default"))))
	set (DLL_EXPORT __attribute__((visibility("default"))))
elseif (${PLATFORM_MAC})
	set (DLL_IMPORT __attribute__((visibility("default"))))
	set (DLL_EXPORT __attribute__((visibility("default"))))
else ()
	set (DLL_IMPORT )
	set (DLL_EXPORT )
endif ()

add_definitions (-DDLLIMPORT ${DLL_IMPORT})
add_definitions (-DDLLEXPORT ${DLL_EXPORT})

add_subdirectory(Core)
add_subdirectory(GameBase)
add_subdirectory(Memory)
add_subdirectory(Thread)