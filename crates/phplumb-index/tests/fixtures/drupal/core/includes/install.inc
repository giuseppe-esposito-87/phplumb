<?php

function install_services($container)
{
    $container->set('installer.only', new \stdClass());
}
