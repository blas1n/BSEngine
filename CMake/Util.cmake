function (register_library)
    get_filename_component (MODULE_NAME ${CMAKE_CURRENT_SOURCE_DIR} NAME)
    
    file (GLOB_RECURSE SRCS "*.cpp")
    add_library (${MODULE_NAME} SHARED ${SRCS})
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