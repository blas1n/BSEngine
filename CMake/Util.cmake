function (register_library)
    get_filename_component (MODULE_NAME ${CMAKE_CURRENT_SOURCE_DIR} NAME)
    
    # This is unnecessary, but added for convenience in the editor.
    file (GLOB_RECURSE INCS "*.h")
    file (GLOB_RECURSE SRCS "*.cpp")
    add_library (${MODULE_NAME} SHARED ${INCS} ${SRCS})
    target_include_directories (${MODULE_NAME} PUBLIC "Public")

    string (TOUPPER ${MODULE_NAME}_API API)
    target_compile_definitions (${MODULE_NAME} PRIVATE ${API}=${DLL_EXPORT} INTERFACE ${API}=${DLL_IMPORT})

    if (EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/Public/pch.h")
        target_precompile_headers (${MODULE_NAME} PUBLIC Public/pch.h)
    endif ()

    if (EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/Private/pch.h")
        target_precompile_headers (${MODULE_NAME} PRIVATE Private/pch.h)
    endif ()
endfunction ()

function (link_test)
    get_filename_component (MODULE_NAME ${CMAKE_CURRENT_SOURCE_DIR} NAME)

    target_include_directories(Tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/Public)
    target_link_libraries(Tests PRIVATE ${MODULE_NAME})
endfunction ()