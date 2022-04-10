
/*
    Welcome to your first dbt model!
    Did you know that you can also configure models directly within SQL files?
    This will override configurations stated in dbt_project.yml

    Try changing "table" to "view" below
*/

with source_data as (

    select 1 as id
    union all
    select null as id

)

select *
{{ config(materialized='table', meta={
    'test': 'something',
}, substitute='abcd', tags=['1', '2']) }}
{{ config(materialized='new_value', meta={
    'test2': 'something else'
}, substitute='efgh', tags=['3', '4']) }}

from source_data

/*
    Uncomment the line below to remove records with null `id` values
*/

-- where id is not null