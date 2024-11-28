use calamine::{
  open_workbook,
  DataType::{Float, String},
  Error, Reader, Xlsx,
};

fn main() -> Result<(), Error> {
  // println!("Hello, world!");
  let path = "a.xlsx";
  let mut workbook: Xlsx<_> = open_workbook(path)?;
  let range = workbook
    .worksheet_range("sheet")
    .ok_or(Error::Msg("Cannot find 'Sheet1'"))??;

  for row in range.rows() {
    // println!("row: {:?}, {:?}, {:?}", row[0], row[1], row[10]);
    if let String(name) = &row[0] {
      if let Float(id) = &row[1] {
        if let Float(price) = &row[10] {
          println!("{},{},{}", name, id, price);
        } else {
          // println!("not match {:?}", row);
        }
      } else {
        // println!("not match {:?}", row);
      }
    } else {
      // println!("not match {:?}", row);
    }
  }

  Ok(())
}

// cargo run  > ~/other_project/docs/doc/充值表.csv
// unix2dos 充值表.csv
// php gen_config.php
