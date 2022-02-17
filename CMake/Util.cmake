function (register_library)
    get_filename_component (MODULE_NAME ${CMAKE_CURRENT_SOURCE_DIR} NAME)
    
    # This is unnecessary, but added for convenience in the editor.
    file (GLOB_RECURSE INCS "*.h")
    file (GLOB_RECURSE SRCS "*.cpp")

    add_library (${MODULE_NAME} SHARED ${INCS} ${SRCS})
    target_include_directories (${MODULE_NAME} PUBLIC "include")

    string (TOUPPER ${MODULE_NAME}_API API)
    target_compile_definitions (${MODULE_NAME} PRIVATE ${API}=${DLL_EXPORT} INTERFACE ${API}=${DLL_IMPORT})

    if (EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/include/pch.h")
        target_precompile_headers (${MODULE_NAME} PUBLIC include/pch.h)
    endif ()

    if (EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/src/pch.h")
        target_precompile_headers (${MODULE_NAME} PRIVATE src/pch.h)
    endif ()
endfunction ()