macro (register_module)
    get_filename_component (MODULE_NAME ${CMAKE_CURRENT_SOURCE_DIR} NAME)
    set (PROJECT_ID ${CMAKE_PROJECT_NAME}-${MODULE_NAME})

    file (GLOB_RECURSE SOURCES CONFIGURE_DEPENDS "Private/*.cpp")
    add_library (${PROJECT_ID} SHARED ${SOURCES})
    target_include_directories (${PROJECT_ID} PUBLIC "Public")

    string (TOUPPER ${MODULE_NAME}_API API)
    target_compile_definitions (${PROJECT_ID} PRIVATE ${API}=${DLL_EXPORT})

    if (EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/Public/pch.h")
        target_precompile_headers (${PROJECT_ID} PUBLIC Public/pch.h)
    endif ()

    if (EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/Private/pch.h")
        target_precompile_headers (${PROJECT_ID} PRIVATE Private/pch.h)
    endif ()
endmacro ()

macro (add_public_dependencies)
	target_link_libraries (${PROJECT_ID} PUBLIC ${ARGN})
#	get_filename_component (PARENT_DIR ${CMAKE_CURRENT_SOURCE_DIR} DIRECTORY)

#	foreach (ARG ${ARGN})
#		target_include_directories (${PROJECT_ID} PUBLIC ${PARENT_DIR}/${ARG})
#	endforeach ()
endmacro ()

macro (add_private_dependencies)
	target_link_libraries (${PROJECT_ID} PRIVATE ${ARGN})
#	get_filename_component (PARENT_DIR ${CMAKE_CURRENT_SOURCE_DIR} DIRECTORY)

#	foreach (ARG ${ARGN})
#		target_include_directories (${PROJECT_ID} PRIVATE ${PARENT_DIR}/${ARG})
#	endforeach ()
endmacro ()