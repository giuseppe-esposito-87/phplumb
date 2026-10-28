<?php

namespace Symfony\Bundle\FrameworkBundle\Controller;

abstract class AbstractController
{
    protected function generateUrl(string $route, array $parameters = []): string
    {
        return '';
    }

    protected function render(string $view, array $parameters = []): object
    {
        return new \stdClass();
    }

    protected function renderView(string $view, array $parameters = []): string
    {
        return '';
    }

    protected function redirectToRoute(string $route, array $parameters = []): object
    {
        return new \stdClass();
    }
}
