pub mod html;
pub mod resolve;
pub mod slashlink;
pub mod transclude;
pub mod write;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
