macro (register_module)
    get_filename_component (MODULE_NAME ${CMAKE_CURRENT_SOURCE_DIR} NAME)
    set (MODULE_ID ${CMAKE_PROJECT_NAME}-${MODULE_NAME})
    
    file(GLOB_RECURSE SRCS "*.cpp")
    add_library (${MODULE_ID} SHARED ${SRCS})
    target_include_directories (${MODULE_ID} PUBLIC "Public")

    string (TOUPPER ${MODULE_NAME}_API API)
    target_compile_definitions (${MODULE_ID} PRIVATE ${API}=${DLL_EXPORT} INTERFACE ${API}=${DLL_IMPORT})

    if (EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/Public/pch.h")
        target_precompile_headers (${MODULE_ID} PUBLIC Public/pch.h)
    endif ()

    if (EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/Private/pch.h")
        target_precompile_headers (${MODULE_ID} PRIVATE Private/pch.h)
    endif ()
endmacro ()